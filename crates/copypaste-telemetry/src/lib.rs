//! `copypaste-telemetry` — opt-in, privacy-first error reporting.
//!
//! # ⚠ Currently unwired — not an active privacy control
//!
//! **This crate is not connected to any caller.** Neither the daemon, CLI, nor
//! UI routes errors through it. The [`PiiScrubber`] therefore never runs in
//! production, and [`SentryReporter`] is never constructed outside tests.
//!
//! Do NOT rely on this crate for PII protection in its current state. Before
//! enabling it, wire a call site in the daemon (or wherever errors should be
//! reported) and confirm consent gating end-to-end on real hardware.
//!
//! # Status
//!
//! 0.3-dev wires the real Sentry SDK behind [`SentryReporter`]. The default
//! ([`ReportConsent::Disabled`]) still ships a [`NoopReporter`] and performs
//! zero I/O; the network path is only reachable when the caller explicitly
//! constructs a [`SentryReporter`] with a DSN **and** an `Enabled*` consent
//! value.
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
//! - The PII scrubber runs on every event *before* it reaches the Sentry
//!   transport. With consent `Disabled` the report is dropped before the
//!   scrubber too, so a disabled reporter is a true no-op.
//!
//! # Sentry SDK configuration
//!
//! When [`SentryReporter::new`] (or [`SentryReporter::with_scrubber`]) is
//! used, the SDK is initialised with:
//!
//! - `send_default_pii = false` — Sentry's automatic IP / user-id capture is
//!   off.
//! - `traces_sample_rate = 0.0` — no performance tracing in beta.
//! - `attach_stacktrace = false` — no implicit backtrace capture.
//! - `release = sentry::release_name!()` — picked from `CARGO_PKG_VERSION` of
//!   the crate that calls `init`, used only for grouping on the server.
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
mod scrubber;

pub use crate::error::{OsTag, ReportableError};
pub use crate::scrubber::PiiScrubber;

use std::sync::Arc;
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

/// Errors a reporter backend can surface.
#[derive(Debug, Error)]
pub enum TelemetryError {
    /// The backend exists in the type system but has no implementation yet.
    /// Retained for API stability; the live Sentry backend never returns it.
    #[error("telemetry backend not implemented in this build")]
    NotImplemented,
    /// The backend rejected the event (e.g. transport failure, invalid DSN).
    /// Free-form `String` so backends can include a static reason without
    /// dragging in extra dependencies.
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

/// Live Sentry backend.
///
/// Constructing a [`SentryReporter`] with [`Self::new`] (or
/// [`Self::with_scrubber`]) initialises the `sentry` crate's global hub and
/// holds the resulting [`sentry::ClientInitGuard`] for the lifetime of the
/// reporter. Dropping the reporter flushes any in-flight events and shuts
/// the transport down.
///
/// Every outbound report passes through [`PiiScrubber`] before reaching the
/// transport. With [`ReportConsent::Disabled`] the report is dropped before
/// the scrubber runs — the reporter then performs no work at all.
///
/// `SentryReporter` is *not* [`Clone`] because the underlying SDK guard owns
/// the global client; share it behind an `Arc` if multiple owners are
/// needed.
pub struct SentryReporter {
    scrubber: Arc<PiiScrubber>,
    consent: ReportConsent,
    // The guard is `Option` so test constructors can omit it (the test
    // harness in `sentry::test::with_captured_events` installs its own
    // client on the current hub). The field is kept named `_guard` to
    // signal that we hold it purely for its `Drop` side-effect.
    _guard: Option<sentry::ClientInitGuard>,
}

impl SentryReporter {
    /// Construct a [`SentryReporter`] that initialises the Sentry SDK with
    /// the supplied DSN.
    ///
    /// The resulting client is configured to never auto-collect PII
    /// (`send_default_pii = false`), to disable performance tracing
    /// (`traces_sample_rate = 0.0`), and to never attach automatic
    /// stacktraces. Holding the returned reporter keeps the global client
    /// alive; dropping it flushes outstanding events and shuts the
    /// transport down.
    ///
    /// Returns [`TelemetryError::BackendError`] if the SDK rejects the DSN
    /// at construction time.
    pub fn new(dsn: &str, consent: ReportConsent) -> Result<Self, TelemetryError> {
        Self::with_scrubber(dsn, consent, Arc::new(PiiScrubber::default()))
    }

    /// Construct a [`SentryReporter`] with a caller-supplied scrubber. Use
    /// this when extra organisation-specific redaction patterns must be
    /// layered on top of the defaults.
    pub fn with_scrubber(
        dsn: &str,
        consent: ReportConsent,
        scrubber: Arc<PiiScrubber>,
    ) -> Result<Self, TelemetryError> {
        // Parse the DSN up-front so a malformed value surfaces as a typed
        // error rather than a silent panic deep inside `sentry::init`.
        let dsn_parsed: sentry::types::Dsn =
            dsn.parse().map_err(|e: sentry::types::ParseDsnError| {
                TelemetryError::BackendError(format!("invalid Sentry DSN: {e}"))
            })?;

        let options = sentry::ClientOptions {
            dsn: Some(dsn_parsed),
            release: sentry::release_name!(),
            // CRITICAL — privacy contract. None of these may flip without
            // updating `docs/privacy/telemetry-policy.md` in the same
            // commit.
            send_default_pii: false,
            traces_sample_rate: 0.0,
            attach_stacktrace: false,
            ..Default::default()
        };

        let guard = sentry::init(options);
        Ok(Self {
            scrubber,
            consent,
            _guard: Some(guard),
        })
    }

    /// Construct a [`SentryReporter`] that does **not** initialise the
    /// Sentry SDK. Intended exclusively for tests that wrap calls in
    /// `sentry::test::with_captured_events` (which installs its own client
    /// on the current hub). Production code MUST use [`Self::new`].
    #[doc(hidden)]
    pub fn for_testing(consent: ReportConsent, scrubber: Arc<PiiScrubber>) -> Self {
        Self {
            scrubber,
            consent,
            _guard: None,
        }
    }

    /// Discard queued events without flushing and tear down the Sentry
    /// transport.
    ///
    /// This is the correct response to a user revoking telemetry consent at
    /// runtime: dropping the [`SentryReporter`] alone would cause the held
    /// [`sentry::ClientInitGuard`] to flush already-queued events to the
    /// transport on `Drop`, contradicting the user's intent. Calling
    /// `shutdown_without_flush` first closes the global client with a
    /// zero-duration timeout so queued events are dropped on the floor; the
    /// subsequent `Drop` of `self` is then a no-op.
    ///
    /// Returns `true` when a live client was closed, `false` when the SDK
    /// was not initialised (e.g. test reporter, or already shut down).
    ///
    /// After this call the reporter is inert — further [`Self::report`]
    /// invocations still consult the `consent` flag (and are typically also
    /// `Disabled` once the caller toggles), but even an erroneous attempt
    /// to send would find no transport on the hub.
    pub fn shutdown_without_flush(&self) -> bool {
        // Zero timeout = drop the queue. `Hub::current().client()` returns
        // an `Arc<Client>` if a client is installed; `close` consumes the
        // client on the hub and returns `true` on a clean shutdown.
        if let Some(client) = sentry::Hub::current().client() {
            client.close(Some(std::time::Duration::ZERO));
            true
        } else {
            false
        }
    }
}

impl ErrorReporter for SentryReporter {
    fn report(&self, event: ReportableError) -> Result<(), TelemetryError> {
        // Consent gate runs first — a disabled reporter performs zero work,
        // not even scrubbing. This matches the privacy contract: the only
        // observable difference between `Disabled` and a `NoopReporter`
        // is the type name.
        if matches!(self.consent, ReportConsent::Disabled) {
            return Ok(());
        }

        // Scrub *before* anything else touches the event so even the local
        // tracing line cannot accidentally surface raw PII to log sinks.
        let event = event.scrubbed(&self.scrubber);

        // Build the message body from the coarse, categorical fields on
        // `ReportableError`. There is intentionally no free-form `context`
        // / `message` field on the type — adding one requires a policy
        // change (see `docs/privacy/telemetry-policy.md`).
        let body = format!(
            "{crate_name}@{crate_version} [{os:?}] {error_class}",
            crate_name = event.crate_name,
            crate_version = event.crate_version,
            os = event.os,
            error_class = event.error_class,
        );

        // `capture_message` is fire-and-forget; the SDK queues internally
        // and the held guard flushes on drop. We swallow the returned UUID
        // because the trait contract does not expose it.
        let _ = sentry::capture_message(&body, sentry::Level::Error);

        tracing::debug!(
            crate_name = %event.crate_name,
            error_class = %event.error_class,
            "SentryReporter dispatched event"
        );
        Ok(())
    }
}

/// Build a **no-op** reporter regardless of `consent`.
///
/// # ⚠ Consent is not honoured — this function always returns a [`NoopReporter`]
///
/// Despite accepting a [`ReportConsent`] argument, `init` never initialises
/// the Sentry SDK and never performs any network I/O, **even when called with
/// [`ReportConsent::EnabledMinimal`] or [`ReportConsent::EnabledFull`]**. The
/// consent value is accepted for API-stability only.
///
/// This is intentional while the telemetry crate remains unwired: no caller
/// in the daemon, CLI, or UI routes errors through it, so there is no risk of
/// accidental PII transmission. When real reporting is needed, use
/// [`init_with_dsn`] (or construct a [`SentryReporter`] directly), which
/// actually gates on `consent` and initialises the transport.
///
/// Returns a boxed [`NoopReporter`] that discards every event.
pub fn init(consent: ReportConsent) -> Box<dyn ErrorReporter> {
    // Explicitly ignored: no DSN is available at this call site, so every
    // consent level maps to a Noop. See the doc-comment above — callers that
    // need real reporting MUST use init_with_dsn instead.
    let _ = consent;
    Box::new(NoopReporter::new())
}

/// Build a reporter for the given consent level, initialising the Sentry
/// SDK with `dsn` if the user has opted in.
///
/// - [`ReportConsent::Disabled`] returns a [`NoopReporter`] regardless of
///   the supplied DSN. No SDK initialisation happens, no network contact.
/// - [`ReportConsent::EnabledMinimal`] / [`EnabledFull`](`ReportConsent::EnabledFull`)
///   return a [`SentryReporter`] holding the SDK guard.
///
/// Returns [`TelemetryError::BackendError`] only when SDK initialisation
/// fails (typically: malformed DSN).
pub fn init_with_dsn(
    consent: ReportConsent,
    dsn: &str,
) -> Result<Box<dyn ErrorReporter>, TelemetryError> {
    match consent {
        ReportConsent::Disabled => Ok(Box::new(NoopReporter::new())),
        ReportConsent::EnabledMinimal | ReportConsent::EnabledFull => {
            let r = SentryReporter::new(dsn, consent)?;
            Ok(Box::new(r))
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
    fn init_without_dsn_returns_noop_for_any_consent() {
        for consent in [
            ReportConsent::Disabled,
            ReportConsent::EnabledMinimal,
            ReportConsent::EnabledFull,
        ] {
            let r = init(consent);
            let evt = ReportableError::new(
                "copypaste-core",
                "0.3.0-dev",
                "test.event",
                OsTag::current(),
            );
            assert!(r.report(evt).is_ok());
        }
    }

    #[test]
    fn init_with_dsn_disabled_returns_noop() {
        // A valid DSN structurally — we never reach the transport because
        // consent is Disabled.
        let r = init_with_dsn(ReportConsent::Disabled, "https://public@sentry.example/1")
            .expect("disabled init never fails");
        let evt = ReportableError::new(
            "copypaste-core",
            "0.3.0-dev",
            "test.event",
            OsTag::current(),
        );
        assert!(r.report(evt).is_ok());
    }

    #[test]
    fn init_with_dsn_rejects_garbage() {
        let result = init_with_dsn(ReportConsent::EnabledFull, "not-a-dsn");
        // `Box<dyn ErrorReporter>` is not `Debug`, so we cannot use
        // `expect_err`. Match on the discriminant directly.
        match result {
            Err(TelemetryError::BackendError(_)) => {}
            Err(other) => panic!("expected BackendError, got {other:?}"),
            Ok(_) => panic!("garbage DSN must be rejected"),
        }
    }
}
