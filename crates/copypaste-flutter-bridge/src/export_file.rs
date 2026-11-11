//! The text-history export file, written as chunks arrive.
//!
//! The bytes are the pretty JSON of one [`ExportData`] followed by a newline,
//! exactly what a single-reply export wrote, so existing files and readers
//! stay compatible. The file appears at its destination only once complete.

use std::io::{self, BufWriter, Write};
use std::path::Path;

use copypaste_fs::PendingFile;
use copypaste_ipc::{ExportData, ExportItem};

pub(crate) struct ExportFile {
    out: BufWriter<PendingFile>,
    items: u64,
}

impl ExportFile {
    pub(crate) fn create(path: &Path) -> io::Result<Self> {
        let mut out = BufWriter::new(PendingFile::create(path)?);
        out.write_all(b"{\n  \"items\": [")?;
        Ok(Self { out, items: 0 })
    }

    pub(crate) fn write_items(&mut self, items: &[ExportItem]) -> io::Result<()> {
        for item in items {
            if self.items > 0 {
                self.out.write_all(b",")?;
            }
            let mut nested = Nested(&mut self.out);
            nested.write_all(b"\n")?;
            serde_json::to_writer_pretty(&mut nested, item)?;
            self.items += 1;
        }
        Ok(())
    }

    /// Close the document, move it into place and return how many items it holds.
    pub(crate) fn finish(mut self, summary: &ExportData) -> io::Result<u64> {
        if self.items > 0 {
            self.out.write_all(b"\n  ")?;
        }
        write!(
            self.out,
            "],\n  \"skipped_non_text\": {},\n  \"skipped_undecryptable\": {}\n}}\n",
            summary.skipped_non_text, summary.skipped_undecryptable
        )?;
        self.out
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?
            .publish()?;
        Ok(self.items)
    }
}

/// Indents pretty JSON by two levels. JSON escapes every newline inside a
/// string, so each raw newline is structural.
struct Nested<W>(W);

impl<W: Write> Write for Nested<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        for (index, line) in bytes.split(|byte| *byte == b'\n').enumerate() {
            if index > 0 {
                self.0.write_all(b"\n    ")?;
            }
            self.0.write_all(line)?;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(content: &str, secret: bool, pinned: bool) -> ExportItem {
        ExportItem {
            privacy: copypaste_ipc::ClipboardPrivacy {
                secret,
                transient: false,
            },
            content: content.into(),
            content_type: "text/plain".into(),
            created_at: 1_700_000_000_000,
            pinned,
        }
    }

    fn written(data: &ExportData, chunks: &[usize]) -> Vec<u8> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let mut file = ExportFile::create(&path).unwrap();
        let mut rest = data.items.as_slice();
        for size in chunks {
            let (chunk, tail) = rest.split_at(*size);
            file.write_items(chunk).unwrap();
            rest = tail;
        }
        file.write_items(rest).unwrap();
        assert!(!path.exists(), "the file appeared before it was complete");
        assert_eq!(file.finish(data).unwrap(), data.items.len() as u64);
        std::fs::read(path).unwrap()
    }

    #[test]
    fn chunked_output_matches_the_single_reply_file_byte_for_byte() {
        let data = ExportData {
            items: vec![
                item("plain", false, true),
                item(
                    "line one\nline two\t\"quoted\" \u{1} ünïcödé 🙂",
                    true,
                    false,
                ),
                item("", false, false),
            ],
            skipped_non_text: 2,
            skipped_undecryptable: 1,
        };
        let mut expected = serde_json::to_vec_pretty(&data).unwrap();
        expected.push(b'\n');

        assert_eq!(written(&data, &[1, 0, 2]), expected);
        assert_eq!(written(&data, &[]), expected);
    }

    #[test]
    fn an_empty_export_matches_the_single_reply_file() {
        let data = ExportData {
            items: Vec::new(),
            skipped_non_text: 0,
            skipped_undecryptable: 0,
        };
        let mut expected = serde_json::to_vec_pretty(&data).unwrap();
        expected.push(b'\n');
        assert_eq!(written(&data, &[]), expected);
    }

    #[test]
    fn an_unfinished_export_leaves_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let mut file = ExportFile::create(&path).unwrap();
        file.write_items(&[item("partial", false, false)]).unwrap();
        drop(file);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
