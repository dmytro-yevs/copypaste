//! Bounded, redacted runtime event logging shared by the app and daemon.

use std::{
    fmt::{self},
    fs,
    io::{self, Read as _},
    path::{Path, PathBuf},
};

use copypaste_ipc::{redact::scrub_paths, LogVerbosity};
use serde::{Deserialize, Serialize};
use tracing::{field::Visit, Event, Subscriber};
use tracing_appender::non_blocking::{NonBlockingBuilder, WorkerGuard};
use tracing_subscriber::{
    fmt::{self as tracing_fmt, format::FormatEvent, format::Writer, FmtContext},
    layer::SubscriberExt as _,
    registry::LookupSpan,
    EnvFilter,
};

const RESOURCE_BUDGETS: &str = include_str!("../../../config/runtime-resource-budgets.json");

#[derive(Clone, Copy, Deserialize)]
struct LogLimits {
    max_files_per_process: usize,
    max_file_bytes: u64,
    export_bytes_per_process: usize,
}
fn log_limits() -> LogLimits {
    #[derive(Deserialize)]
    struct Budgets {
        runtime_logs: LogLimits,
    }
    serde_json::from_str::<Budgets>(RESOURCE_BUDGETS)
        .expect("runtime log limits must be valid")
        .runtime_logs
}
const BUFFERED_LINES: usize = 1_024;
pub const MAX_PAGE_SIZE: usize = 100;

mod level;
mod query;
mod reader;
mod writer;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use level::set_level;
pub use query::list;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Process {
    App,
    Daemon,
}

/// A deliberately small, redacted runtime event that may enter the application UI.
///
/// Event fields are excluded at write time. This reader only accepts the
/// formatter's fixed four-column shape and bounds both bytes and rows before
/// it serialises anything to UI code.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeEvent {
    pub timestamp_ms: u64,
    pub level: LogLevel,
    pub process: Process,
    pub target: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "ERROR" => Some(Self::Error),
            "WARN" => Some(Self::Warn),
            "INFO" => Some(Self::Info),
            "DEBUG" => Some(Self::Debug),
            "TRACE" => Some(Self::Trace),
            _ => None,
        }
    }
}

/// The UI may ask for a short page only.
///
/// `cursor` is opaque. Join a fresh head read to a paged one *through it* and
/// never by comparing timestamps: a page boundary may fall inside a
/// millisecond, so "older than the oldest row on screen" discards rows that
/// were never shown.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLogQuery {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub level: Option<LogLevel>,
    #[serde(default)]
    pub process: Option<Process>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default = "default_page_size")]
    pub limit: usize,
}

fn default_page_size() -> usize {
    50
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeLogPage {
    pub events: Vec<RuntimeEvent>,
    pub next_cursor: Option<String>,
}

impl Process {
    pub(crate) fn prefix(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Daemon => "daemon",
        }
    }
}

pub struct RuntimeLogGuard {
    _worker: WorkerGuard,
}

/// Installs the one process-wide tracing subscriber and retains its worker.
///
/// The formatter intentionally records only literal event messages. Error,
/// clipboard, pairing and other ad-hoc fields can contain private data, so
/// they never reach the disk sink in the first place.
pub fn init(log_dir: &Path, process: Process) -> anyhow::Result<RuntimeLogGuard> {
    fs::create_dir_all(log_dir)?;
    let appender = writer::BoundedLog::open(log_dir, process, log_limits())?;
    let (writer, worker) = NonBlockingBuilder::default()
        .buffered_lines_limit(BUFFERED_LINES)
        .lossy(false)
        .finish(appender);
    // An explicit `RUST_LOG` is a developer override and outranks the setting.
    let overridden = EnvFilter::try_from_default_env().ok();
    let pinned = overridden.is_some();
    let default = LogVerbosity::default();
    let (filter, control) = level::reloadable(
        overridden.unwrap_or_else(|| level::filter(default)),
        default,
    );
    let subscriber = tracing_subscriber::registry().with(filter).with(
        tracing_fmt::layer()
            .event_format(SafeEventFormat)
            .with_ansi(false)
            .with_writer(writer),
    );
    tracing::subscriber::set_global_default(subscriber)?;
    if !pinned {
        level::install(control);
    }
    log_panic_locations();
    Ok(RuntimeLogGuard { _worker: worker })
}

/// Records where a panic happened, never its payload, which may hold data.
fn log_panic_locations() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        match info.location() {
            Some(at) => tracing::error!("panic at {}:{}", at.file(), at.line()),
            None => tracing::error!("panic at an unknown location"),
        }
        previous(info);
    }));
}

/// Write a bounded support section from only known runtime-log filenames.
/// Never expose their locations; the native save command receives just bytes.
/// Files are read and scrubbed one at a time, so the section is never held
/// in memory whole.
pub fn export(log_dir: &Path, process: Process, out: &mut dyn io::Write) -> io::Result<()> {
    let prefix = format!("{}.", process.prefix());
    let entries = match fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".log"))
        })
        .collect();
    files.sort();

    let budget = log_limits().export_bytes_per_process;
    let mut written = 0;
    let mut ends_line = false;
    let mut remaining = budget;
    for file in files.into_iter().rev() {
        if remaining == 0 {
            break;
        }
        let mut bytes = Vec::new();
        fs::File::open(&file)?
            .take(remaining as u64)
            .read_to_end(&mut bytes)?;
        let take = bytes.len();
        let safe = scrub_paths(&String::from_utf8_lossy(&bytes));
        drop(bytes);
        let mut end = safe.len().min(budget - written);
        while !safe.is_char_boundary(end) {
            end -= 1;
        }
        out.write_all(&safe.as_bytes()[..end])?;
        written += end;
        if end > 0 {
            ends_line = safe.as_bytes()[end - 1] == b'\n';
        }
        if !ends_line && written < budget {
            out.write_all(b"\n")?;
            written += 1;
            ends_line = true;
        }
        remaining -= take;
    }
    Ok(())
}

pub(crate) fn parse_event(line: &str, process: Process) -> Option<RuntimeEvent> {
    let mut pieces = line.splitn(4, ' ');
    let timestamp_ms = pieces.next()?.parse().ok()?;
    let level = LogLevel::parse(pieces.next()?)?;
    let target = pieces.next()?.trim_end_matches(':');
    let message = pieces.next()?.trim();
    if target.is_empty() || message.is_empty() {
        return None;
    }
    Some(RuntimeEvent {
        timestamp_ms,
        level,
        process,
        target: scrub_paths(target),
        message: scrub_paths(message),
    })
}

#[derive(Clone, Copy)]
struct SafeEventFormat;

impl<S, N> FormatEvent<S, N> for SafeEventFormat
where
    S: Subscriber + for<'span> LookupSpan<'span>,
    N: for<'writer> tracing_subscriber::fmt::FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut message = Message::default();
        event.record(&mut message);
        let metadata = event.metadata();
        let timestamp_ms = copypaste_clock::now_ms();
        writeln!(
            writer,
            "{timestamp_ms} {} {}: {}",
            metadata.level(),
            metadata.target(),
            scrub_paths(message.text.as_deref().unwrap_or(metadata.name()))
        )
    }
}

#[derive(Default)]
struct Message {
    text: Option<String>,
}

impl Visit for Message {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.record(field.name(), value);
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        // Every other field is dropped, so it is never formatted either.
        if field.name() == "message" {
            self.record(field.name(), format!("{value:?}").trim_matches('"'));
        }
    }
}

impl Message {
    fn record(&mut self, name: &str, value: &str) {
        if name == "message" {
            self.text = Some(value.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Corpus {
        display_leak_cases: Vec<DisplayCase>,
        safe_display_cases: Vec<SafeCase>,
        #[serde(default)]
        redaction_cases: Vec<RedactionCase>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct DisplayCase {
        id: String,
        surface: String,
        expected_surface: String,
        forbidden_fragments: Vec<String>,
    }

    #[derive(Deserialize)]
    struct SafeCase {
        id: String,
        surface: String,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RedactionCase {
        id: String,
        surface: String,
        expected_surface: String,
        forbidden_fragments: Vec<String>,
    }

    fn corpus() -> Corpus {
        serde_json::from_str(include_str!(
            "../../../test-support/security/path-security-vectors.json"
        ))
        .expect("path security corpus is valid")
    }

    fn export_surface(surface: &str) -> String {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("daemon.2026-08-01.log"), surface).unwrap();
        exported(directory.path(), Process::Daemon)
    }

    fn exported(dir: &Path, process: Process) -> String {
        let mut out = Vec::new();
        export(dir, process, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn export_reads_only_the_configured_budget_from_large_legacy_files() {
        use std::io::{Seek, SeekFrom, Write};
        let dir = tempfile::tempdir().unwrap();
        let mut file = fs::File::create(dir.path().join("app.2026-01-01.log")).unwrap();
        file.write_all(b"bounded export\n").unwrap();
        file.seek(SeekFrom::Start(512 * 1024 * 1024)).unwrap();
        file.write_all(b"must not read this tail\n").unwrap();
        let out = exported(dir.path(), Process::App);
        assert!(out.starts_with("bounded export\n"));
        assert!(!out.contains("must not read this tail"));
        assert!(out.len() <= log_limits().export_bytes_per_process);
    }

    #[test]
    fn export_redacts_a_path_and_ignores_unrelated_files() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("daemon.2026-08-01.log"),
            "opened /Users/alice/private.db\n",
        )
        .unwrap();
        fs::write(directory.path().join("other.log"), "do not export").unwrap();

        let text = exported(directory.path(), Process::Daemon);
        assert!(text.contains("<path>"));
        assert!(!text.contains("alice"));
        assert!(!text.contains("do not export"));
    }

    #[test]
    fn formatter_boundary_rejects_a_secret_event_field() {
        let mut message = Message::default();
        message.record("error", "token=secret-value");
        message.record("message", "capture failed");

        assert_eq!(message.text.as_deref(), Some("capture failed"));
        assert_ne!(message.text.as_deref(), Some("token=secret-value"));
    }

    #[derive(Clone, Default)]
    struct Buffer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl io::Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl<'a> tracing_fmt::MakeWriter<'a> for Buffer {
        type Writer = Self;
        fn make_writer(&'a self) -> Self {
            self.clone()
        }
    }

    /// Runs `body` under the production formatter and a reloadable filter.
    fn capture_live(body: impl FnOnce(&level::LevelControl)) -> String {
        let buffer = Buffer::default();
        let start = LogVerbosity::Info;
        let (filter, control) = level::reloadable(level::filter(start), start);
        let subscriber = tracing_subscriber::registry().with(filter).with(
            tracing_fmt::layer()
                .event_format(SafeEventFormat)
                .with_ansi(false)
                .with_writer(buffer.clone()),
        );
        tracing::subscriber::with_default(subscriber, || {
            tracing::callsite::rebuild_interest_cache();
            body(&control);
        });
        let bytes = buffer.0.lock().unwrap().clone();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn a_level_change_applies_to_the_next_event() {
        let log = capture_live(|control| {
            tracing::debug!(target: "copypaste_probe", "hidden at info");
            assert!(control.apply(LogVerbosity::Debug));
            assert!(!control.apply(LogVerbosity::Debug), "unchanged is a no-op");
            tracing::debug!(target: "copypaste_probe", "shown at debug");
            tracing::debug!(target: "dependency", "a dependency stays at info");
            assert!(control.apply(LogVerbosity::Error));
            tracing::warn!(target: "copypaste_probe", "hidden at error");
            tracing::error!(target: "copypaste_probe", "shown at error");
        });

        assert!(
            log.contains("DEBUG copypaste_probe: shown at debug"),
            "{log}"
        );
        assert!(
            log.contains("ERROR copypaste_probe: shown at error"),
            "{log}"
        );
        for hidden in ["hidden at info", "a dependency", "hidden at error"] {
            assert!(!log.contains(hidden), "{hidden} leaked into {log}");
        }
    }

    #[test]
    fn debug_keeps_only_the_redacted_message() {
        let log = capture_live(|control| {
            control.apply(LogVerbosity::Debug);
            tracing::debug!(
                target: "copypaste_probe",
                token = "secret-token",
                content = "copied text",
                "item read from /Users/alice/private.db"
            );
        });

        assert!(log.contains("item read from <path>"), "{log}");
        for secret in ["secret-token", "copied text", "alice"] {
            assert!(!log.contains(secret), "{secret} leaked into {log}");
        }
    }

    #[test]
    fn a_dropped_field_is_never_formatted() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FORMATTED: AtomicUsize = AtomicUsize::new(0);
        struct Probe;
        impl fmt::Debug for Probe {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                FORMATTED.fetch_add(1, Ordering::Relaxed);
                formatter.write_str("probe")
            }
        }

        let log = capture_live(|_| {
            tracing::warn!(target: "copypaste_probe", detail = ?Probe, "kept message");
        });

        assert!(log.contains("WARN copypaste_probe: kept message"), "{log}");
        assert_eq!(FORMATTED.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn runtime_event_serializes_timestamp_as_a_number() {
        let event = RuntimeEvent {
            timestamp_ms: i64::MAX as u64,
            level: LogLevel::Info,
            process: Process::App,
            target: "copypaste::capture".to_owned(),
            message: "capture started".to_owned(),
        };

        let json = serde_json::to_value(event).unwrap();
        assert_eq!(json["timestamp_ms"], serde_json::json!(i64::MAX));
    }

    #[test]
    fn support_log_export_path_security_corpus() {
        let corpus = corpus();
        for case in corpus.display_leak_cases {
            let rendered = export_surface(&case.surface);
            assert_eq!(
                rendered,
                format!("{}\n", case.expected_surface),
                "{} rendered unexpectedly",
                case.id
            );
            for fragment in case.forbidden_fragments {
                assert!(
                    !rendered.contains(&fragment),
                    "{} kept a forbidden fragment",
                    case.id
                );
            }
        }
        for case in corpus.safe_display_cases {
            assert_eq!(
                export_surface(&case.surface),
                format!("{}\n", case.surface),
                "{} was changed",
                case.id
            );
        }
        for case in corpus.redaction_cases {
            let rendered = export_surface(&case.surface);
            assert_eq!(
                rendered,
                format!("{}\n", case.expected_surface),
                "{} rendered unexpectedly",
                case.id
            );
            for fragment in case.forbidden_fragments {
                assert!(
                    !rendered.contains(&fragment),
                    "{} kept a forbidden fragment",
                    case.id
                );
            }
        }
    }
}
