//! Bounded, redacted runtime event logging shared by the app and daemon.

use std::{
    fmt::{self},
    fs,
    io::{self, Read as _, Seek as _, SeekFrom},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use copypaste_ipc::redact::scrub_paths;
use serde::{Deserialize, Serialize};
use tracing::{field::Visit, Event, Subscriber};
use tracing_appender::{
    non_blocking::{NonBlockingBuilder, WorkerGuard},
    rolling::{RollingFileAppender, Rotation},
};
use tracing_subscriber::{
    fmt::{self as tracing_fmt, format::FormatEvent, format::Writer, FmtContext},
    layer::SubscriberExt as _,
    registry::LookupSpan,
    EnvFilter,
};

const MAX_LOG_FILES: usize = 7;
const BUFFERED_LINES: usize = 1_024;
const EXPORT_BYTES: usize = 1_000_000;
const VIEW_BYTES: usize = 1_000_000;
const VIEW_EVENTS: usize = 2_000;
pub const MAX_PAGE_SIZE: usize = 100;
const MAX_QUERY_LENGTH: usize = 160;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Process {
    App,
    Daemon,
}

/// A deliberately small, redacted runtime event that may enter the WebView.
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

/// The UI may ask for a short page only. `cursor` is an opaque filtered-row
/// offset, deliberately not a filesystem position or filename.
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
    fn prefix(self) -> &'static str {
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
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(process.prefix())
        .filename_suffix("log")
        .max_log_files(MAX_LOG_FILES)
        .build(log_dir)?;
    let (writer, worker) = NonBlockingBuilder::default()
        .buffered_lines_limit(BUFFERED_LINES)
        .lossy(false)
        .finish(appender);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = tracing_subscriber::registry().with(filter).with(
        tracing_fmt::layer()
            .event_format(SafeEventFormat)
            .with_ansi(false)
            .with_writer(writer),
    );
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(RuntimeLogGuard { _worker: worker })
}

/// Produce a bounded support section from only known runtime-log filenames.
/// Never expose their locations; the native save command receives just bytes.
pub fn export(log_dir: &Path, process: Process) -> io::Result<String> {
    let prefix = format!("{}.", process.prefix());
    let entries = match fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(String::new()),
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

    let mut out = String::new();
    let mut remaining = EXPORT_BYTES;
    for file in files.into_iter().rev() {
        if remaining == 0 {
            break;
        }
        let bytes = fs::read(&file)?;
        let take = bytes.len().min(remaining);
        let text = String::from_utf8_lossy(&bytes[..take]);
        out.push_str(&scrub_paths(&text));
        if !out.ends_with('\n') {
            out.push('\n');
        }
        remaining -= take;
    }
    Ok(out)
}

/// Read a bounded page of the same redacted events included in support
/// bundles. The reader never returns a log filename, absolute path, tracing
/// field or unbounded file content.
pub fn list(
    log_dir: &Path,
    query: &RuntimeLogQuery,
    include_daemon: bool,
) -> io::Result<RuntimeLogPage> {
    let limit = query.limit.clamp(1, MAX_PAGE_SIZE);
    let offset = query
        .cursor
        .as_deref()
        .and_then(|cursor| cursor.parse::<usize>().ok())
        .unwrap_or_default();
    let needle = query
        .query
        .as_deref()
        .unwrap_or_default()
        .chars()
        .take(MAX_QUERY_LENGTH)
        .collect::<String>()
        .to_lowercase();

    let mut events = Vec::new();
    for process in [Process::App, Process::Daemon] {
        if process == Process::Daemon && !include_daemon {
            continue;
        }
        if query.process.is_some_and(|wanted| wanted != process) {
            continue;
        }
        events.extend(read_process_events(log_dir, process)?);
    }
    events.sort_by_key(|event| std::cmp::Reverse(event.timestamp_ms));
    events.retain(|event| {
        query.level.is_none_or(|level| event.level == level)
            && (needle.is_empty()
                || event.target.to_lowercase().contains(&needle)
                || event.message.to_lowercase().contains(&needle))
    });

    let end = offset.saturating_add(limit).min(events.len());
    let page = events.get(offset..end).unwrap_or_default().to_vec();
    Ok(RuntimeLogPage {
        events: page,
        next_cursor: (end < events.len()).then(|| end.to_string()),
    })
}

fn read_process_events(log_dir: &Path, process: Process) -> io::Result<Vec<RuntimeEvent>> {
    let prefix = format!("{}.", process.prefix());
    let entries = match fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
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

    let mut remaining = VIEW_BYTES;
    let mut events = Vec::new();
    for file in files.into_iter().rev() {
        if remaining == 0 || events.len() >= VIEW_EVENTS {
            break;
        }
        let (bytes, starts_mid_line) = read_tail(&file, remaining)?;
        remaining = remaining.saturating_sub(bytes.len());
        let text = String::from_utf8_lossy(&bytes);
        // A tail can start mid-line. Dropping that one fragment stops a
        // malformed partial event from being presented as a real record.
        let mut lines = text.lines();
        if starts_mid_line {
            lines.next();
        }
        for line in lines.rev() {
            if let Some(event) = parse_event(line, process) {
                events.push(event);
                if events.len() >= VIEW_EVENTS {
                    break;
                }
            }
        }
    }
    Ok(events)
}

fn read_tail(path: &Path, remaining: usize) -> io::Result<(Vec<u8>, bool)> {
    let mut file = fs::File::open(path)?;
    let length = file.metadata()?.len();
    let take = length.min(remaining as u64) as usize;
    file.seek(SeekFrom::Start(length.saturating_sub(take as u64)))?;
    let mut bytes = Vec::with_capacity(take);
    file.take(take as u64).read_to_end(&mut bytes)?;
    Ok((bytes, (take as u64) < length))
}

fn parse_event(line: &str, process: Process) -> Option<RuntimeEvent> {
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
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
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
        self.record(field.name(), format!("{value:?}").trim_matches('"'));
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

    #[test]
    fn export_redacts_a_path_and_ignores_unrelated_files() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("daemon.2026-08-01.log"),
            "opened /Users/alice/private.db\n",
        )
        .unwrap();
        fs::write(directory.path().join("other.log"), "do not export").unwrap();

        let exported = export(directory.path(), Process::Daemon).unwrap();
        assert!(exported.contains("<path>"));
        assert!(!exported.contains("alice"));
        assert!(!exported.contains("do not export"));
    }

    #[test]
    fn formatter_boundary_rejects_a_secret_event_field() {
        let mut message = Message::default();
        message.record("error", "token=secret-value");
        message.record("message", "capture failed");

        assert_eq!(message.text.as_deref(), Some("capture failed"));
        assert_ne!(message.text.as_deref(), Some("token=secret-value"));
    }

    #[test]
    fn list_pages_only_fixed_format_redacted_events() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("daemon.2026-08-01.log"),
            "100 INFO copypaste::capture: capture started\n200 WARN copypaste::storage: opened /Users/alice/private.db\nnot an event\n",
        )
        .unwrap();
        let query = RuntimeLogQuery {
            cursor: None,
            level: None,
            process: Some(Process::Daemon),
            query: None,
            limit: 1,
        };

        let first = list(directory.path(), &query, true).unwrap();
        assert_eq!(first.events.len(), 1);
        assert_eq!(first.events[0].timestamp_ms, 200);
        assert!(first.events[0].message.contains("<path>"));
        assert!(!first.events[0].message.contains("alice"));
        assert_eq!(first.next_cursor.as_deref(), Some("1"));

        let next = list(
            directory.path(),
            &RuntimeLogQuery {
                cursor: first.next_cursor,
                ..query
            },
            true,
        )
        .unwrap();
        assert_eq!(next.events[0].timestamp_ms, 100);
    }

    #[test]
    fn list_never_invents_a_daemon_on_android() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("daemon.2026-08-01.log"),
            "100 INFO copypaste::capture: capture started\n",
        )
        .unwrap();
        let page = list(
            directory.path(),
            &RuntimeLogQuery {
                cursor: None,
                level: None,
                process: None,
                query: None,
                limit: 50,
            },
            false,
        )
        .unwrap();
        assert!(page.events.is_empty());
    }
}
