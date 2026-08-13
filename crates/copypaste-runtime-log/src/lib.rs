//! Bounded, redacted runtime event logging shared by the app and daemon.

use std::{
    fmt::{self},
    fs, io,
    path::{Path, PathBuf},
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
pub const MAX_PAGE_SIZE: usize = 100;

mod query;
mod reader;

pub use query::list;

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
}
