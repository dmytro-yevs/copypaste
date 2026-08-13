//! Log capture, so a test can assert on what staging actually emits.
//!
//! The staging diagnostics are the one place where filenames, content ids and
//! decrypted bytes could reach a log file, so "it contains no path" has to be
//! an assertion rather than a reading of the source.

use std::io;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub(super) struct CapturedLog(Arc<Mutex<Vec<u8>>>);

impl CapturedLog {
    /// Runs `body` with every event on this thread captured, and returns them.
    pub(super) fn record<T>(&self, body: impl FnOnce() -> T) -> String {
        let subscriber = tracing_subscriber::fmt()
            .with_writer(self.clone())
            .with_ansi(false)
            .finish();
        tracing::subscriber::with_default(subscriber, body);
        String::from_utf8_lossy(&self.buffer()).into_owned()
    }

    fn buffer(&self) -> Vec<u8> {
        self.0
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .clone()
    }
}

impl io::Write for CapturedLog {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl tracing_subscriber::fmt::MakeWriter<'_> for CapturedLog {
    type Writer = Self;

    fn make_writer(&self) -> Self::Writer {
        self.clone()
    }
}
