//! Opt-in test support for the actual message-only formatter at default INFO.

use std::io;
use std::sync::{Arc, Mutex};

use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

use super::SafeEventFormat;

#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);
impl io::Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl<'a> fmt::MakeWriter<'a> for Buffer {
    type Writer = Self;
    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}

/// Capture this thread using the production formatter and default INFO filter.
/// No process log file, runtime initialization, or environment mutation occurs.
pub fn capture<T>(body: impl FnOnce() -> T) -> (T, String) {
    let buffer = Buffer::default();
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::new("info"))
        .with(
            fmt::layer()
                .event_format(SafeEventFormat)
                .with_ansi(false)
                .with_writer(buffer.clone()),
        );
    let result = tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        body()
    });
    let text = String::from_utf8(buffer.0.lock().unwrap().clone()).expect("formatter writes UTF-8");
    (result, text)
}
