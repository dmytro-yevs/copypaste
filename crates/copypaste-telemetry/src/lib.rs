//! `copypaste-telemetry` — opt-in, privacy-first error reporting (stub).
//!
//! # Status
//!
//! 0.2-beta ships only the API surface and a no-op default. The
//! [`SentryReporter`] (and any other backend) is intentionally a stub that
//! returns [`TelemetryError::NotImplemented`] from [`ErrorReporter::report`]
//! and is wired up in a later release. This lets downstream crates depend on
//! the trait today without locking in a backend.
//!
//! # Defaults
//!
//! - Reporting is **off** unless the user explicitly chooses
//!   [`ReportConsent::EnabledMinimal`] or [`ReportConsent::EnabledFull`].
//! - [`init`] called with [`ReportConsent::Disabled`] returns a
//!   [`NoopReporter`] that swallows every event and never panics.
//! - There is no implicit/automatic opt-in path. The caller (CLI / UI /
//!   daemon) is responsible for surfacing a consent prompt and persisting
//!   the choice. This crate never reads or writes any consent state.
//!
//! # Privacy
//!
//! See [`docs/privacy/telemetry-policy.md`][policy] in the repository for the
//! authoritative description of what may be sent, retention, and the user's
//! rights. The [`ReportableError`] type's documentation also describes the
//! anonymization contract enforced at the type level.
//!
//! [policy]: https://github.com/dmytro/CopyPaste/blob/main/docs/privacy/telemetry-policy.md

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;

pub use crate::error::{OsTag, ReportableError};

use thiserror::Error;

/// User consent for outbound telemetry. Default is [`Self::Disabled`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ReportConsent {
    /// Reporting is fully disabled. [`init`] returns a [`NoopReporter`].
    #[default]
    Disabled,
    /// User has opted in to a minimal set (error class + crate + OS tag).
    /// Currently identical to [`Self::EnabledFull`] because no extra fields
    /// exist yet — the distinction reserves room for future opt-in extras
    /// (e.g. local timezone offset bucket) without breaking the API.
    EnabledMinimal,
    /// User has opted in to the full event payload. Equivalent to
    /// [`Self::EnabledMinimal`] in 0.2-beta; reserved for future expansion.
    EnabledFull,
}

/// Errors a reporter backend can surface. Stubs return
/// [`Self::NotImplemented`] until the backend is wired up.
#[derive(Debug, Error)]
pub enum TelemetryError {
    /// The backend exists in the type system but has no implementation yet.
    #[error("telemetry backend not implemented in this build")]
    NotImplemented,
    /// The backend rejected the event (e.g. transport failure). Free-form
    /// `String` so backends can include a static reason without dragging in
    /// extra dependencies.
    #[error("telemetry backend failed: {0}")]
    BackendError(String),
}

/// Sink for anonymized error events.
///
/// Implementations MUST:
/// 1. Never panic, even on backend failure.
/// 2. Never block the caller; reporting is expected to be fire-and-forget
///    from the caller's perspective (backends may queue internally).
/// 3. Never read or write user payload data beyond what is in
///    [`ReportableError`].
pub trait ErrorReporter: Send + Sync {
    /// Submit an anonymized event. Returning `Err` is for backend bookkeeping
    /// only — callers typically log and discard the error.
    fn report(&self, event: ReportableError) -> Result<(), TelemetryError>;
}

/// Default reporter. Accepts every event and discards it. Free of side
/// effects and safe to construct from any thread.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopReporter;

impl NoopReporter {
    /// Construct a [`NoopReporter`].
    pub const fn new() -> Self {
        Self
    }
}

impl ErrorReporter for NoopReporter {
    fn report(&self, _event: ReportableError) -> Result<(), TelemetryError> {
        // Intentionally a no-op. Tracing left out to avoid surprising the
        // operator when reporting is disabled.
        Ok(())
    }
}

/// Stub Sentry backend. Returns [`TelemetryError::NotImplemented`] from
/// [`ErrorReporter::report`]. The constructor exists so downstream crates can
/// pin to the type today; wiring lands in a later release.
#[derive(Debug, Default, Clone, Copy)]
pub struct SentryReporter;

impl SentryReporter {
    /// Construct a stub [`SentryReporter`]. Does not contact any network.
    pub const fn new() -> Self {
        Self
    }
}

impl ErrorReporter for SentryReporter {
    fn report(&self, event: ReportableError) -> Result<(), TelemetryError> {
        // Trace at debug so accidental invocations during dev are visible
        // without spamming production logs.
        tracing::debug!(
            crate_name = %event.crate_name,
            error_class = %event.error_class,
            "SentryReporter stub invoked; not implemented in 0.2-beta"
        );
        Err(TelemetryError::NotImplemented)
    }
}

/// Build a reporter for the given consent level.
///
/// Returns a boxed trait object so the caller can store it behind a single
/// type regardless of the chosen backend. With [`ReportConsent::Disabled`]
/// this is guaranteed to be a [`NoopReporter`] and to perform zero I/O.
pub fn init(consent: ReportConsent) -> Box<dyn ErrorReporter> {
    match consent {
        ReportConsent::Disabled => Box::new(NoopReporter::new()),
        // Both opt-in variants currently route to the stub Sentry backend.
        // When a real backend lands, this match is the single place to swap
        // implementations or to differentiate Minimal vs Full payloads.
        ReportConsent::EnabledMinimal | ReportConsent::EnabledFull => {
            Box::new(SentryReporter::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_consent_is_disabled() {
        assert_eq!(ReportConsent::default(), ReportConsent::Disabled);
    }

    #[test]
    fn init_disabled_returns_working_noop() {
        let r = init(ReportConsent::Disabled);
        let evt = ReportableError::new(
            "copypaste-core",
            "0.2.0-beta.0",
            "test.event",
            OsTag::current(),
        );
        assert!(r.report(evt).is_ok());
    }

    #[test]
    fn init_enabled_returns_stub_that_errors() {
        let r = init(ReportConsent::EnabledFull);
        let evt = ReportableError::new(
            "copypaste-core",
            "0.2.0-beta.0",
            "test.event",
            OsTag::current(),
        );
        assert!(matches!(r.report(evt), Err(TelemetryError::NotImplemented)));
    }
}
