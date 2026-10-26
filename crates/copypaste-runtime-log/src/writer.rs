//! Size-bounded append-only segments preserve log-reader byte-offset identity.
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use crate::{reader::log_files, LogLimits, Process};

pub(super) struct BoundedLog {
    directory: PathBuf,
    process: Process,
    limits: LogLimits,
    file: Option<File>,
    key: String,
    bytes: u64,
}

impl BoundedLog {
    pub(super) fn open(directory: &Path, process: Process, limits: LogLimits) -> io::Result<Self> {
        if limits.max_files_per_process == 0 || limits.max_file_bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid log limits",
            ));
        }
        let mut writer = Self {
            directory: directory.into(),
            process,
            limits,
            file: None,
            key: String::new(),
            bytes: 0,
        };
        writer.rotate(&today())?;
        Ok(writer)
    }

    fn rotate(&mut self, day: &str) -> io::Result<()> {
        let files = log_files(&self.directory, self.process)?;
        // Keep names monotonic even after a clock correction. Never overwrite
        // or rename a segment: existing viewer cursors must remain meaningful.
        let latest = files.last().map(|file| file.day.as_str()).unwrap_or(day);
        let day = day.max(&latest[..10]);
        let sequence = files
            .last()
            .filter(|file| file.day.starts_with(day))
            .and_then(|file| file.day.split_once('.'))
            .map(|(_, part)| part.parse::<u32>().unwrap_or(0))
            .unwrap_or(0)
            + 1;
        if sequence > 999_999 {
            return Err(io::Error::other("log segment sequence exhausted"));
        }
        let key = format!("{day}.{sequence:06}");
        let path = self
            .directory
            .join(format!("{}.{}.log", self.process.prefix(), key));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        // Open first, so a permission/storage failure never removes old logs.
        let file = options.open(path)?;
        self.file = Some(file);
        self.key = key;
        self.bytes = 0;
        // Close the old file before removing it, including on Windows. Old
        // oversized daily logs are retired when adopting the bounded format.
        let keep_old = self.limits.max_files_per_process - 1;
        let mut kept = 0;
        for old in files.into_iter().rev() {
            if old.path.metadata()?.len() > self.limits.max_file_bytes || kept >= keep_old {
                std::fs::remove_file(old.path)?;
            } else {
                kept += 1;
            }
        }
        Ok(())
    }

    fn write_at(&mut self, bytes: &[u8], day: &str) -> io::Result<usize> {
        // Rotate between records, never inside a UTF-8 message or log line.
        // Reject an oversized individual event instead of violating the cap.
        if bytes.len() as u64 > self.limits.max_file_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "log event exceeds file limit",
            ));
        }
        if self.bytes + bytes.len() as u64 > self.limits.max_file_bytes || day > &self.key[..10] {
            self.rotate(day)?;
        }
        let file = self.file.as_mut().expect("log file opened");
        if let Err(error) = file.write_all(bytes) {
            self.bytes = file.metadata()?.len();
            return Err(error);
        }
        self.bytes += bytes.len() as u64;
        Ok(bytes.len())
    }
}

impl Write for BoundedLog {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_at(bytes, &today())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.as_mut().expect("log file opened").flush()
    }
}

fn today() -> String {
    let (year, month, day) = time::OffsetDateTime::now_utc().to_calendar_date();
    format!("{year:04}-{:02}-{day:02}", month as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{list, RuntimeLogQuery};

    #[test]
    fn size_rotation_retains_newest_events_and_paging_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let limits = LogLimits {
            max_files_per_process: 3,
            max_file_bytes: 80,
            export_bytes_per_process: 100,
        };
        let mut writer = BoundedLog::open(dir.path(), Process::App, limits).unwrap();
        for index in 0..8 {
            writer
                .write_all(
                    format!("1000 INFO test: event {index} padding padding padding\n").as_bytes(),
                )
                .unwrap();
        }
        let files = log_files(dir.path(), Process::App).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files
            .iter()
            .all(|file| file.path.metadata().unwrap().len() <= 80));
        let query = RuntimeLogQuery {
            cursor: None,
            level: None,
            process: Some(Process::App),
            query: None,
            limit: 1,
        };
        let first = list(dir.path(), &query, false).unwrap();
        assert!(first.events[0].message.starts_with("event 7"));
        drop(writer);
        let mut writer = BoundedLog::open(dir.path(), Process::App, limits).unwrap();
        writer.write_all(b"1001 INFO test: event 8\n").unwrap();
        let next = list(
            dir.path(),
            &RuntimeLogQuery {
                cursor: first.next_cursor,
                ..query
            },
            false,
        )
        .unwrap();
        assert!(next.events[0].message.starts_with("event 6"));
        assert_eq!(log_files(dir.path(), Process::App).unwrap().len(), 3);
    }

    #[test]
    fn oversized_legacy_logs_are_retired_without_touching_other_processes() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("app.2026-01-01.log");
        std::fs::write(&old, [b'x'; 81]).unwrap();
        let other = dir.path().join("daemon.2026-01-01.log");
        std::fs::write(&other, "preserve").unwrap();
        let limits = LogLimits {
            max_files_per_process: 3,
            max_file_bytes: 80,
            export_bytes_per_process: 100,
        };
        let mut writer = BoundedLog::open(dir.path(), Process::App, limits).unwrap();
        assert!(!old.exists());
        assert_eq!(std::fs::read_to_string(other).unwrap(), "preserve");
        assert!(writer.write_at(&[b'x'; 81], &today()).is_err());
        writer
            .write_at(b"1001 WARN test: recovery\n", "9999-01-01")
            .unwrap();
        assert_eq!(writer.key, "9999-01-01.000001");
        writer
            .write_at(b"1002 INFO test: clock corrected\n", "2026-01-01")
            .unwrap();
        assert_eq!(writer.key, "9999-01-01.000001");
    }
}
