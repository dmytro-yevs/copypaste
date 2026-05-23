//! Panic boundary for UniFFI-exported functions.
//!
//! UniFFI-exported Rust functions cross the JNI boundary into a managed JVM.
//! A bare Rust panic that unwinds through that boundary aborts the JVM
//! process (looks like a hard crash to the user, no recovery path). This
//! module wraps exported function bodies with [`std::panic::catch_unwind`]
//! so panics become typed `CopypasteError::Panicked(msg)` values that Kotlin
//! callers can match and surface gracefully.
//!
//! v0.3 — addresses THREAT-MODEL OI-7 (operational integrity: process
//! survives unexpected panics in the Rust core).
//!
//! ## Usage
//!
//! For `Result`-returning UniFFI exports (preferred — Kotlin sees the error):
//!
//! ```ignore
//! pub fn encrypt_text(...) -> Result<EncryptedBlob, CopypasteError> {
//!     panic_boundary::catch_result(|| {
//!         // original body
//!     })
//! }
//! ```
//!
//! For exports that return a plain value (panic becomes
//! `PanicError::Panicked` — caller must adapt or change signature):
//!
//! ```ignore
//! pub fn pure_getter() -> i32 {
//!     panic_boundary::catch(|| 42).unwrap_or(0)
//! }
//! ```

use std::panic::{catch_unwind, AssertUnwindSafe};

/// Error returned when a wrapped closure panics.
///
/// Carries the panic payload's string representation (if recoverable) or a
/// generic placeholder for non-string payloads. We deliberately do not
/// include backtraces or other process-internal detail in the message —
/// Kotlin surfaces this to users.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PanicError {
    /// A Rust panic was caught at the FFI boundary.
    #[error("Rust panic: {0}")]
    Panicked(String),
}

/// Extract a human-readable message from a panic payload, falling back to a
/// generic string if the payload is neither `&'static str` nor `String`.
fn payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "Rust panic with non-string payload".to_string()
    }
}

/// Run `f`, catching any panic and converting it to [`PanicError::Panicked`].
///
/// Use this for closures that return a plain `T`. The caller decides how to
/// surface or recover from the panic.
///
/// `AssertUnwindSafe` is required because most closures we wrap capture
/// `&mut` state or types that are not `UnwindSafe`; we accept that risk
/// because the alternative is a JVM-killing abort, which is strictly worse.
pub fn catch<F, T>(f: F) -> Result<T, PanicError>
where
    F: FnOnce() -> T,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => Ok(value),
        Err(payload) => Err(PanicError::Panicked(payload_to_string(payload))),
    }
}

/// Run a `Result`-returning closure: a panic OR an `Err` both surface as
/// `Err`. Requires the target error type to implement `From<PanicError>`.
///
/// This is the preferred wrapper for UniFFI exports that already return
/// `Result<T, E>` — the caller never observes a distinct "panic vs error"
/// case, just a single error channel.
pub fn catch_result<F, T, E>(f: F) -> Result<T, E>
where
    F: FnOnce() -> Result<T, E>,
    E: From<PanicError>,
{
    match catch(f) {
        Ok(inner) => inner,
        Err(panic) => Err(panic.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A local error type that implements `From<PanicError>` so we can
    // exercise `catch_result` without dragging in `CopypasteError`.
    #[derive(Debug, thiserror::Error)]
    enum TestError {
        #[error("normal error: {0}")]
        Normal(String),
        #[error("panicked: {0}")]
        Panicked(String),
    }

    impl From<PanicError> for TestError {
        fn from(p: PanicError) -> Self {
            match p {
                PanicError::Panicked(m) => TestError::Panicked(m),
            }
        }
    }

    #[test]
    fn catches_string_panic() {
        let result = catch(|| {
            let owned: String = "boom-string".to_string();
            panic!("{}", owned);
        });
        match result {
            Err(PanicError::Panicked(msg)) => assert!(msg.contains("boom-string"), "got: {msg}"),
            Ok(_) => panic!("expected panic to be caught"),
        }
    }

    #[test]
    fn catches_static_str_panic() {
        let result: Result<(), PanicError> = catch(|| panic!("boom-static"));
        match result {
            Err(PanicError::Panicked(msg)) => assert_eq!(msg, "boom-static"),
            Ok(_) => panic!("expected panic to be caught"),
        }
    }

    #[test]
    fn catches_non_string_panic_with_fallback() {
        let result: Result<(), PanicError> = catch(|| {
            // Panic with a non-string payload (an integer).
            std::panic::panic_any(42u64);
        });
        match result {
            Err(PanicError::Panicked(msg)) => {
                assert_eq!(msg, "Rust panic with non-string payload");
            }
            Ok(_) => panic!("expected panic to be caught"),
        }
    }

    #[test]
    fn passes_through_normal_return() {
        let result = catch(|| 7i32 + 35);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn passes_through_err_return() {
        let result: Result<i32, TestError> =
            catch_result(|| Err(TestError::Normal("nominal".into())));
        match result {
            Err(TestError::Normal(m)) => assert_eq!(m, "nominal"),
            other => panic!("expected Normal err, got {other:?}"),
        }
    }

    #[test]
    fn catch_result_converts_panic_to_target_err() {
        let result: Result<i32, TestError> = catch_result(|| -> Result<i32, TestError> {
            panic!("inside-result");
        });
        match result {
            Err(TestError::Panicked(m)) => assert!(m.contains("inside-result"), "got: {m}"),
            other => panic!("expected Panicked, got {other:?}"),
        }
    }

    #[test]
    fn catch_result_passes_through_ok() {
        let result: Result<i32, TestError> = catch_result(|| Ok(99));
        assert_eq!(result.unwrap(), 99);
    }
}
