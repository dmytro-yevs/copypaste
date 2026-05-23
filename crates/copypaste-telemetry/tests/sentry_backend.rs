//! Integration tests for the live `SentryReporter` backend.
//!
//! These pin the network-facing contract of the telemetry crate:
//!
//! - `ReportConsent::Disabled` drops every report before the scrubber and
//!   before the SDK is consulted.
//! - `ReportConsent::EnabledFull` (and `EnabledMinimal`) dispatch exactly
//!   one event per `report()` call.
//! - Events are scrubbed *before* leaving the reporter — the captured
//!   envelope must contain no PII even when the original `error_class`
//!   carried some.
//! - A panic inside the report path never propagates to the caller.
//! - `NoopReporter` is unaffected by SDK initialisation.
//!
//! The tests use `sentry::test::with_captured_events`, which installs a
//! local in-memory transport on a fresh hub for the duration of the
//! closure. No network traffic is generated.

use std::sync::Arc;

use copypaste_telemetry::{
    ErrorReporter, NoopReporter, OsTag, PiiScrubber, ReportConsent, ReportableError, SentryReporter,
};

fn sample_event() -> ReportableError {
    ReportableError::new(
        "copypaste-daemon",
        "0.3.0-dev",
        "ipc.parse_error",
        OsTag::MacOs,
    )
}

/// Helper: build a reporter that does NOT touch the global hub, so the
/// test transport installed by `with_captured_events` is the one consulted.
fn reporter_for_test(consent: ReportConsent) -> SentryReporter {
    SentryReporter::for_testing(consent, Arc::new(PiiScrubber::default()))
}

#[test]
fn consent_disabled_drops_report() {
    let reporter = reporter_for_test(ReportConsent::Disabled);

    let events = sentry::test::with_captured_events(|| {
        for _ in 0..16 {
            assert!(reporter.report(sample_event()).is_ok());
        }
    });

    assert!(
        events.is_empty(),
        "Disabled consent must not dispatch any events, got {} ({:?})",
        events.len(),
        events
    );
}

#[test]
fn consent_enabled_sends_report() {
    let reporter = reporter_for_test(ReportConsent::EnabledFull);

    let events = sentry::test::with_captured_events(|| {
        assert!(reporter.report(sample_event()).is_ok());
    });

    assert_eq!(
        events.len(),
        1,
        "Enabled consent must dispatch exactly one event"
    );
    let evt = &events[0];
    assert_eq!(evt.level, sentry::Level::Error);
    let msg = evt
        .message
        .as_deref()
        .expect("capture_message attaches a message body");
    // Body shape: "<crate>@<version> [<os>] <error_class>"
    assert!(msg.contains("copypaste-daemon"), "got: {msg:?}");
    assert!(msg.contains("0.3.0-dev"), "got: {msg:?}");
    assert!(msg.contains("ipc.parse_error"), "got: {msg:?}");
    assert!(msg.contains("MacOs"), "got: {msg:?}");
}

#[test]
fn consent_enabled_minimal_also_sends() {
    // EnabledMinimal currently has the same wire shape as EnabledFull; this
    // test pins that the consent gate treats both as "send".
    let reporter = reporter_for_test(ReportConsent::EnabledMinimal);

    let events = sentry::test::with_captured_events(|| {
        assert!(reporter.report(sample_event()).is_ok());
    });

    assert_eq!(events.len(), 1);
}

#[test]
fn report_is_scrubbed_before_send() {
    let reporter = reporter_for_test(ReportConsent::EnabledFull);

    // Feed an error_class that carries an email, a home-dir path, and a
    // UUID — all of which the default scrubber must redact.
    let dirty = ReportableError::new(
        "copypaste-daemon",
        "0.3.0-dev",
        "login alice@example.com from /Users/alice/db.sqlite \
         token=550e8400-e29b-41d4-a716-446655440000 failed",
        OsTag::MacOs,
    );

    let events = sentry::test::with_captured_events(|| {
        assert!(reporter.report(dirty).is_ok());
    });

    assert_eq!(events.len(), 1);
    let msg = events[0]
        .message
        .as_deref()
        .expect("capture_message attaches a message body");

    // Redaction markers present.
    assert!(msg.contains("<REDACTED-EMAIL>"), "got: {msg:?}");
    assert!(msg.contains("~/"), "got: {msg:?}");
    assert!(msg.contains("<REDACTED-HEX>"), "got: {msg:?}");

    // Raw PII absent.
    assert!(!msg.contains("alice@example.com"), "got: {msg:?}");
    assert!(!msg.contains("/Users/alice"), "got: {msg:?}");
    assert!(!msg.contains("550e8400"), "got: {msg:?}");
}

#[test]
fn panic_during_report_does_not_propagate() {
    // The reporter's report() path must never panic on any input. We feed
    // pathological strings (empty fields, very long payloads, control
    // characters) and catch_unwind across the call to assert no unwind
    // crosses the FFI boundary into the test runner.
    let reporter = reporter_for_test(ReportConsent::EnabledFull);

    let pathological = ReportableError::new(
        "", // empty crate name
        "", // empty version
        "\x00\x01control\nchars\r\n\t".to_string() + &"x".repeat(8192),
        OsTag::Unknown,
    );

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sentry::test::with_captured_events(|| {
            let _ = reporter.report(pathological);
        })
    }));

    assert!(
        result.is_ok(),
        "report() unwound across catch_unwind boundary"
    );
}

#[test]
fn noop_reporter_remains_unaffected_by_init() {
    // Build a SentryReporter (test variant — no global init) and use a
    // NoopReporter alongside it. The NoopReporter must continue to swallow
    // every event and produce zero envelopes, even when called from
    // inside a hub that has a test transport installed.
    let _sentry = reporter_for_test(ReportConsent::EnabledFull);
    let noop = NoopReporter::new();

    let events = sentry::test::with_captured_events(|| {
        for _ in 0..32 {
            assert!(noop.report(sample_event()).is_ok());
        }
    });

    assert!(
        events.is_empty(),
        "NoopReporter must never produce envelopes, got {}",
        events.len()
    );
}
