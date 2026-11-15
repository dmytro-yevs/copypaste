//! Paging the runtime log: where a page resumes, and what it may cost.

use std::{io, path::Path};

use crate::reader::{EventStream, Position};
use crate::{LogLevel, Process, RuntimeEvent, RuntimeLogPage, RuntimeLogQuery, MAX_PAGE_SIZE};

/// The longest a single call walks before it hands the caller a cursor.
///
/// It bounds work, not reach: a filter that matches nothing returns an empty
/// page *with* a cursor, so the viewer's load-more continues from exactly where
/// this call stopped rather than treating the budget as the end of the log.
const SCAN_EVENTS: usize = 2_000;

const MAX_QUERY_LENGTH: usize = 160;

/// Where each process's stream resumes.
///
/// A process the cursor does not name has already been walked to its end. The
/// position is a byte offset, which an append-only file never moves, rather
/// than a count of rows shown — a row written while the viewer is open lands
/// inside the millisecond a count is measured against, so the next page skips
/// it and repeats rows the reader has already seen (`CopyPaste-8ebg.57`).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Cursor {
    app: Option<Position>,
    daemon: Option<Position>,
}

impl Cursor {
    /// Rejects anything it did not write. A cursor that fails to parse is not
    /// silently treated as "start from the top": a load-more that restarted
    /// would repeat the whole log as if it were new events.
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let mut cursor = Self::default();
        for segment in raw.split(';') {
            let (key, value) = segment.split_once('=')?;
            let position = Position::parse(value)?;
            let slot = match key {
                "a" => &mut cursor.app,
                "d" => &mut cursor.daemon,
                _ => return None,
            };
            if slot.replace(position).is_some() {
                return None;
            }
        }
        (cursor != Self::default()).then_some(cursor)
    }

    pub(crate) fn encode(&self) -> String {
        [(("a"), self.app.as_ref()), (("d"), self.daemon.as_ref())]
            .into_iter()
            .filter_map(|(key, position)| position.map(|at| format!("{key}={}", at.encode())))
            .collect::<Vec<_>>()
            .join(";")
    }

    fn at(&self, process: Process) -> Option<&Position> {
        match process {
            Process::App => self.app.as_ref(),
            Process::Daemon => self.daemon.as_ref(),
        }
    }

    fn set(&mut self, process: Process, position: Position) {
        match process {
            Process::App => self.app = Some(position),
            Process::Daemon => self.daemon = Some(position),
        }
    }
}

/// Read a bounded page of the same redacted events included in support
/// bundles. The reader never returns a log filename, absolute path, tracing
/// field or unbounded file content.
///
/// Blocking file I/O. Callers on an async runtime must put it on a blocking
/// thread — ADR-0011 promises the log sink never stalls the reactor, and a
/// viewer that reads a megabyte of log on it would break that promise from the
/// read side instead of the write side.
pub fn list(
    log_dir: &Path,
    query: &RuntimeLogQuery,
    include_daemon: bool,
) -> io::Result<RuntimeLogPage> {
    let limit = query.limit.clamp(1, MAX_PAGE_SIZE);
    let cursor =
        match query.cursor.as_deref() {
            None => None,
            Some(raw) => Some(Cursor::parse(raw).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "unrecognised cursor")
            })?),
        };
    let needle = query
        .query
        .as_deref()
        .unwrap_or_default()
        .chars()
        .take(MAX_QUERY_LENGTH)
        .collect::<String>()
        .to_lowercase();

    let mut streams = Vec::new();
    for process in [Process::App, Process::Daemon] {
        if process == Process::Daemon && !include_daemon {
            continue;
        }
        if query.process.is_some_and(|wanted| wanted != process) {
            continue;
        }
        match &cursor {
            None => streams.push((process, EventStream::open(log_dir, process, None)?)),
            Some(cursor) => {
                if let Some(at) = cursor.at(process) {
                    streams.push((process, EventStream::open(log_dir, process, Some(at))?));
                }
            }
        }
    }

    let mut events = Vec::with_capacity(limit);
    let mut scanned = 0;
    while events.len() < limit && scanned < SCAN_EVENTS {
        let mut newest: Option<(usize, u64)> = None;
        for (index, (_, stream)) in streams.iter_mut().enumerate() {
            let Some(event) = stream.peek()? else {
                continue;
            };
            let at = event.timestamp_ms;
            // Strictly newer, so two processes writing in the same millisecond
            // always interleave the same way twice.
            if newest.is_none_or(|(_, best)| at > best) {
                newest = Some((index, at));
            }
        }
        let Some((index, _)) = newest else { break };
        let event = streams[index]
            .1
            .take()?
            .expect("the stream just reported this event");
        scanned += 1;
        if matches(&event, query.level, &needle) {
            events.push(event);
        }
    }

    let mut next = Cursor::default();
    for (process, stream) in &mut streams {
        if let Some(position) = stream.position()? {
            next.set(*process, position);
        }
    }
    let next_cursor = (next != Cursor::default()).then(|| next.encode());
    Ok(RuntimeLogPage {
        events,
        next_cursor,
    })
}

fn matches(event: &RuntimeEvent, level: Option<LogLevel>, needle: &str) -> bool {
    level.is_none_or(|level| event.level == level)
        && (needle.is_empty()
            || event.target.to_lowercase().contains(needle)
            || event.message.to_lowercase().contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn query(limit: usize, cursor: Option<&str>) -> RuntimeLogQuery {
        RuntimeLogQuery {
            cursor: cursor.map(str::to_owned),
            level: None,
            process: None,
            query: None,
            limit,
        }
    }

    fn same_ms_rows(range: std::ops::Range<usize>) -> String {
        range
            .map(|n| format!("500 INFO copypaste::capture: row {n}\n"))
            .collect()
    }

    /// Walk to the end, returning every message in the order it was shown.
    fn walk(dir: &Path, limit: usize) -> Vec<String> {
        let mut seen = Vec::new();
        let mut cursor = None;
        for _ in 0..1_000 {
            let page = list(dir, &query(limit, cursor.as_deref()), true).unwrap();
            seen.extend(page.events.iter().map(|event| event.message.clone()));
            let Some(next) = page.next_cursor else { break };
            cursor = Some(next);
        }
        seen
    }

    fn unique(seen: &[String]) -> usize {
        let mut sorted = seen.to_vec();
        sorted.sort();
        sorted.dedup();
        sorted.len()
    }

    #[test]
    fn list_pages_only_fixed_format_redacted_events() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("daemon.2026-08-01.log"),
            "100 INFO copypaste::capture: capture started\n200 WARN copypaste::storage: opened /Users/alice/private.db\nnot an event\n",
        )
        .unwrap();
        let paged = RuntimeLogQuery {
            process: Some(Process::Daemon),
            ..query(1, None)
        };

        let first = list(directory.path(), &paged, true).unwrap();
        assert_eq!(first.events.len(), 1);
        assert_eq!(first.events[0].timestamp_ms, 200);
        assert!(first.events[0].message.contains("<path>"));
        assert!(!first.events[0].message.contains("alice"));

        let next = list(
            directory.path(),
            &RuntimeLogQuery {
                cursor: first.next_cursor,
                ..paged
            },
            true,
        )
        .unwrap();
        assert_eq!(next.events[0].timestamp_ms, 100);
    }

    /// The defect this cursor exists for, at the millisecond that made a count
    /// wrong. Two hundred rows share one timestamp; a hundred are read; a
    /// hundred more arrive bearing that same timestamp while the viewer is
    /// open. A count of "how many at this millisecond were shown" then skips
    /// the new rows and hands back the old ones a second time.
    #[test]
    fn a_write_inside_the_cursor_millisecond_neither_repeats_nor_drops_a_row() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("daemon.2026-08-01.log");
        fs::write(&log, same_ms_rows(0..200)).unwrap();

        let first = list(directory.path(), &query(100, None), true).unwrap();
        assert_eq!(first.events.len(), 100);
        let mut seen: Vec<String> = first.events.iter().map(|e| e.message.clone()).collect();

        // The concurrent write: same millisecond, so it is indistinguishable
        // from the already-shown rows by timestamp alone.
        let mut appended = fs::read_to_string(&log).unwrap();
        appended.push_str(&same_ms_rows(200..300));
        fs::write(&log, appended).unwrap();

        let mut cursor = first.next_cursor;
        while let Some(raw) = cursor {
            let page = list(directory.path(), &query(100, Some(&raw)), true).unwrap();
            seen.extend(page.events.iter().map(|e| e.message.clone()));
            cursor = page.next_cursor;
        }

        assert_eq!(seen.len(), 200, "{}", seen.len());
        assert_eq!(unique(&seen), 200, "a row was shown twice");
        for n in 0..200 {
            assert!(
                seen.contains(&format!("row {n}")),
                "row {n} was never shown"
            );
        }
        // The rows written during the walk are newer than the cursor, so they
        // belong above it. A fresh read is where they appear — never spliced
        // into a walk already under way, which is what would repeat a row.
        let head = list(directory.path(), &query(100, None), true).unwrap();
        assert_eq!(head.events[0].message, "row 299");
    }

    /// One call reads a bounded window. The window is not the end of the log:
    /// a cursor must carry the walk past it, or every row older than the
    /// budget is unreachable from the viewer.
    #[test]
    fn paging_advances_past_the_single_call_scan_budget() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("daemon.2026-08-01.log"),
            same_ms_rows(0..2_100),
        )
        .unwrap();

        let seen = walk(directory.path(), MAX_PAGE_SIZE);

        assert_eq!(seen.len(), 2_100);
        assert_eq!(unique(&seen), 2_100);
    }

    /// Page through a log that is being written the whole time.
    ///
    /// With a row-offset cursor this failed the way `CopyPaste-8ebg.57`
    /// describes: each new line pushed the list down, so page two repeated a
    /// row already shown and dropped one that was never shown.
    #[test]
    fn paging_under_concurrent_writes_repeats_and_skips_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("daemon.2026-08-01.log");
        let original: Vec<String> = (0..20)
            .map(|n| format!("{} INFO copypaste::capture: event {n}", 1_000 + n))
            .collect();
        fs::write(&log, format!("{}\n", original.join("\n"))).unwrap();

        let mut seen = Vec::new();
        let mut cursor = None;
        let mut newer = 0;
        loop {
            let page = list(directory.path(), &query(3, cursor.as_deref()), true).unwrap();
            for event in &page.events {
                seen.push(event.message.clone());
            }
            let Some(next) = page.next_cursor else { break };
            cursor = Some(next);

            // A write lands between every pair of pages, which is the whole
            // point: the viewer is open while the daemon is logging.
            newer += 1;
            let mut appended = fs::read_to_string(&log).unwrap();
            appended.push_str(&format!(
                "{} INFO copypaste::capture: arrived later {newer}\n",
                2_000 + newer
            ));
            fs::write(&log, appended).unwrap();
        }

        let originals: Vec<&String> = seen
            .iter()
            .filter(|message| message.starts_with("event "))
            .collect();
        assert_eq!(originals.len(), 20, "a row was lost or repeated: {seen:?}");
    }

    /// Many events inside one millisecond is the ordinary case for a log, and
    /// the boundary between two pages lands in the middle of one.
    #[test]
    fn a_page_boundary_inside_one_millisecond_advances() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("daemon.2026-08-01.log"),
            same_ms_rows(0..7),
        )
        .unwrap();

        let seen = walk(directory.path(), 3);
        assert_eq!(seen.len(), 7, "{seen:?}");
        assert_eq!(unique(&seen), 7, "a row inside the millisecond repeated");
    }

    /// A cursor this reader did not write must be refused. Treating it as
    /// "start from the top" would make a load-more render the whole log again,
    /// which is indistinguishable from a burst of new events.
    #[test]
    fn an_unrecognised_cursor_is_refused_rather_than_restarted() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("app.2026-08-01.log"),
            "100 INFO copypaste::capture: only event\n",
        )
        .unwrap();

        for raw in [
            "1",
            "",
            "abc",
            "100:",
            ":2",
            "100:2:3",
            "-1:0",
            "a=2026-08-01",
            "x=2026-08-01@1",
            "a=2026-08-01@1;a=2026-08-01@2",
            "a=../secrets@1",
        ] {
            let error = list(directory.path(), &query(50, Some(raw)), true)
                .expect_err(&format!("cursor {raw:?} was accepted"));
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{raw}");
            assert!(!error.to_string().contains('/'), "{raw}");
        }
    }

    /// A filter narrows what is listed; it must not change what a cursor
    /// means. The cursor is a position in the log, so it stays valid across one.
    #[test]
    fn a_cursor_is_a_position_in_the_log_rather_than_in_the_filtered_rows() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("daemon.2026-08-01.log"),
            "100 INFO copypaste::capture: a\n200 WARN copypaste::capture: b\n\
             300 INFO copypaste::capture: c\n400 WARN copypaste::capture: d\n",
        )
        .unwrap();

        let first = list(directory.path(), &query(1, None), true).unwrap();
        assert_eq!(first.events[0].timestamp_ms, 400);

        let filtered = RuntimeLogQuery {
            level: Some(LogLevel::Info),
            ..query(50, first.next_cursor.as_deref())
        };
        let page = list(directory.path(), &filtered, true).unwrap();
        let stamps: Vec<u64> = page.events.iter().map(|event| event.timestamp_ms).collect();
        assert_eq!(stamps, [300, 100], "{stamps:?}");
    }

    /// Two processes writing in the same millisecond must interleave the same
    /// way on every call, or a page boundary between them repeats a row.
    #[test]
    fn both_processes_merge_into_one_stable_order() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("app.2026-08-01.log"),
            "500 INFO copypaste::ui: app one\n500 INFO copypaste::ui: app two\n",
        )
        .unwrap();
        fs::write(
            directory.path().join("daemon.2026-08-01.log"),
            "500 INFO copypaste::capture: daemon one\n500 INFO copypaste::capture: daemon two\n",
        )
        .unwrap();

        let seen = walk(directory.path(), 1);
        assert_eq!(seen.len(), 4);
        assert_eq!(unique(&seen), 4);
        assert_eq!(seen, walk(directory.path(), 3));
    }

    #[test]
    fn list_never_invents_a_daemon_on_android() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("daemon.2026-08-01.log"),
            "100 INFO copypaste::capture: capture started\n",
        )
        .unwrap();

        let page = list(directory.path(), &query(50, None), false).unwrap();
        assert!(page.events.is_empty());
        assert_eq!(page.next_cursor, None);
    }
}
