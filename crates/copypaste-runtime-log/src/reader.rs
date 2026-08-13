//! Reading one process's log files newest event first, in bounded windows.
//!
//! A log file is append-only, so the byte at which a line starts never moves.
//! That offset is the identity [`crate::query`] pages on. A count of rows
//! cannot be one: a row written while the viewer is open lands *inside* the
//! millisecond a count is measured against, and the next page then skips it and
//! repeats rows the reader has already shown.

use std::{
    fs,
    io::{self, Read as _, Seek as _, SeekFrom},
    path::{Path, PathBuf},
};

use crate::{parse_event, Process, RuntimeEvent};

/// The largest slice of one log file held in memory at once.
const WINDOW_BYTES: u64 = 256 * 1024;

/// Where a stream resumes: the byte before which every unread event lies.
///
/// `day` is the date `tracing_appender` puts in the filename, not the filename
/// and not the directory. It survives rotation, sorts chronologically, and
/// discloses nothing about where the logs live (AGENTS.md rule 4).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Position {
    pub(crate) day: String,
    pub(crate) end: u64,
}

impl Position {
    pub(crate) fn encode(&self) -> String {
        format!("{}@{}", self.day, self.end)
    }

    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let (day, end) = raw.split_once('@')?;
        if !is_day(day) {
            return None;
        }
        Some(Self {
            day: day.to_owned(),
            end: end.parse().ok()?,
        })
    }
}

/// `YYYY-MM-DD`, checked by shape so a cursor cannot smuggle a path fragment
/// into the filename comparison below.
fn is_day(value: &str) -> bool {
    value.len() == 10
        && value.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

struct LogFile {
    day: String,
    path: PathBuf,
}

fn log_files(log_dir: &Path, process: Process) -> io::Result<Vec<LogFile>> {
    let prefix = format!("{}.", process.prefix());
    let entries = match fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut files: Vec<LogFile> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter_map(|path| {
            let day = path
                .file_name()?
                .to_str()?
                .strip_prefix(&prefix)?
                .strip_suffix(".log")?;
            is_day(day).then(|| LogFile {
                day: day.to_owned(),
                path: path.clone(),
            })
        })
        .collect();
    // Lexicographic on `YYYY-MM-DD` is chronological, oldest first.
    files.sort_by(|left, right| left.day.cmp(&right.day));
    Ok(files)
}

/// One process's events, newest first.
///
/// File order is the authority within a process: it is the true append order,
/// and a wall clock that steps backwards would otherwise reorder rows that were
/// written in sequence. Timestamps only decide how two processes interleave.
pub(crate) struct EventStream {
    process: Process,
    files: Vec<LogFile>,
    /// The file being read, or `None` once every file is exhausted.
    index: Option<usize>,
    /// Exclusive upper bound of everything still unread, and what
    /// [`EventStream::position`] reports. Always a line boundary.
    bound: u64,
    /// Where the next backward window stops. Never above `bound`.
    scan_end: u64,
    /// File order, so popping the end walks the window newest first. Each
    /// event carries the offset its line starts at.
    buffered: Vec<(RuntimeEvent, u64)>,
}

impl EventStream {
    /// Open at the newest event, or at `from` when resuming a page.
    ///
    /// A `from` naming a day that has rotated away is an exhausted stream
    /// rather than an error: those rows are gone, and reporting a bad cursor
    /// for a log that simply aged out would send the viewer back to the top.
    pub(crate) fn open(
        log_dir: &Path,
        process: Process,
        from: Option<&Position>,
    ) -> io::Result<Self> {
        let files = log_files(log_dir, process)?;
        let (index, bound) = match from {
            None => match files.len() {
                0 => (None, 0),
                count => (Some(count - 1), file_len(&files[count - 1].path)?),
            },
            Some(position) => match files.iter().position(|file| file.day == position.day) {
                Some(index) => (Some(index), position.end.min(file_len(&files[index].path)?)),
                None => (None, 0),
            },
        };
        Ok(Self {
            process,
            files,
            index,
            bound,
            scan_end: bound,
            buffered: Vec::new(),
        })
    }

    pub(crate) fn peek(&mut self) -> io::Result<Option<&RuntimeEvent>> {
        self.fill()?;
        Ok(self.buffered.last().map(|(event, _)| event))
    }

    pub(crate) fn take(&mut self) -> io::Result<Option<RuntimeEvent>> {
        self.fill()?;
        let Some((event, start)) = self.buffered.pop() else {
            return Ok(None);
        };
        // The line just returned becomes the exclusive bound: every event a
        // later page still owes the viewer is strictly before it.
        self.bound = start;
        Ok(Some(event))
    }

    /// Where a later call resumes, or `None` when nothing is left.
    pub(crate) fn position(&mut self) -> io::Result<Option<Position>> {
        if self.peek()?.is_none() {
            return Ok(None);
        }
        let index = self.index.expect("a peeked event came from a file");
        Ok(Some(Position {
            day: self.files[index].day.clone(),
            end: self.bound,
        }))
    }

    fn fill(&mut self) -> io::Result<()> {
        while self.buffered.is_empty() {
            let Some(index) = self.index else {
                return Ok(());
            };
            if self.scan_end == 0 {
                self.index = index.checked_sub(1);
                let length = match self.index {
                    Some(older) => file_len(&self.files[older].path)?,
                    None => 0,
                };
                self.bound = length;
                self.scan_end = length;
                continue;
            }
            self.read_window(index)?;
        }
        Ok(())
    }

    fn read_window(&mut self, index: usize) -> io::Result<()> {
        let end = self.scan_end;
        let wanted = end.saturating_sub(WINDOW_BYTES);
        // One byte before the window, so a boundary that already sits on a line
        // start is not mistaken for a line this window cut in half.
        let from = wanted.saturating_sub(u64::from(wanted > 0));
        let mut bytes = read_range(&self.files[index].path, from, end)?;

        let mut base = from;
        if from > 0 {
            match bytes.iter().position(|byte| *byte == b'\n') {
                Some(cut) => {
                    base = from + cut as u64 + 1;
                    bytes.drain(..=cut);
                }
                // A line longer than the whole window holds no complete record.
                // Dropped rather than presented half-parsed.
                None => bytes.clear(),
            }
        }

        let mut offset = base;
        for line in bytes.split(|byte| *byte == b'\n') {
            let start = offset;
            offset += line.len() as u64 + 1;
            if let Ok(text) = std::str::from_utf8(line) {
                if let Some(event) = parse_event(text, self.process) {
                    self.buffered.push((event, start));
                }
            }
        }
        // `from` rather than `base` when the window held no line start, so a
        // file of unparseable bytes still walks to its beginning and stops.
        self.scan_end = if base > from { base } else { from };
        Ok(())
    }
}

fn file_len(path: &Path) -> io::Result<u64> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.len()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error),
    }
}

fn read_range(path: &Path, from: u64, end: u64) -> io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(from))?;
    let take = end.saturating_sub(from);
    let mut bytes = Vec::with_capacity(take.min(WINDOW_BYTES + 1) as usize);
    file.take(take).read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, day: &str, lines: &[String]) {
        fs::write(
            dir.join(format!("daemon.{day}.log")),
            format!("{}\n", lines.join("\n")),
        )
        .unwrap();
    }

    fn rows(count: usize) -> Vec<String> {
        (0..count)
            .map(|n| format!("500 INFO copypaste::capture: row {n}"))
            .collect()
    }

    fn drain(stream: &mut EventStream) -> Vec<String> {
        let mut seen = Vec::new();
        while let Some(event) = stream.take().unwrap() {
            seen.push(event.message);
        }
        seen
    }

    #[test]
    fn events_arrive_newest_first_across_files() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "2026-08-01", &["100 INFO t: older".to_owned()]);
        write(dir.path(), "2026-08-02", &["200 INFO t: newer".to_owned()]);

        let mut stream = EventStream::open(dir.path(), Process::Daemon, None).unwrap();
        assert_eq!(drain(&mut stream), ["newer", "older"]);
    }

    /// The file is deliberately larger than one window: a reader that only ever
    /// looked at the newest window would stop at its edge and call that the end
    /// of the log.
    #[test]
    fn a_file_larger_than_one_window_is_read_to_its_start() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "2026-08-01", &rows(20_000));

        let mut stream = EventStream::open(dir.path(), Process::Daemon, None).unwrap();
        assert_eq!(drain(&mut stream).len(), 20_000);
    }

    #[test]
    fn a_position_resumes_exactly_where_it_stopped() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "2026-08-01", &rows(500));

        let mut stream = EventStream::open(dir.path(), Process::Daemon, None).unwrap();
        let mut seen = Vec::new();
        for _ in 0..40 {
            seen.push(stream.take().unwrap().unwrap().message);
        }
        let position = stream.position().unwrap().expect("more to read");

        let mut resumed = EventStream::open(dir.path(), Process::Daemon, Some(&position)).unwrap();
        seen.extend(drain(&mut resumed));

        let mut unique = seen.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(seen.len(), 500);
        assert_eq!(unique.len(), 500, "resuming repeated or lost a row");
    }

    /// A cursor older than the retention window names a file that is gone. The
    /// rows it pointed at no longer exist, so the stream is over.
    #[test]
    fn a_position_naming_a_rotated_away_day_is_the_end_of_the_stream() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "2026-08-09", &["100 INFO t: kept".to_owned()]);

        let gone = Position {
            day: "2026-08-01".to_owned(),
            end: 40,
        };
        let mut stream = EventStream::open(dir.path(), Process::Daemon, Some(&gone)).unwrap();
        assert!(stream.take().unwrap().is_none());
    }

    #[test]
    fn a_position_round_trips_and_refuses_a_path() {
        let position = Position {
            day: "2026-08-01".to_owned(),
            end: 4_096,
        };
        assert_eq!(Position::parse(&position.encode()), Some(position));
        for raw in ["", "@1", "2026-08-01@", "../etc@1", "2026-8-01@1", "x@1"] {
            assert!(Position::parse(raw).is_none(), "{raw}");
        }
    }
}
