//! Opt-out is the default and must remain silent under all inputs.
//!
//! These tests pin the privacy contract: with [`ReportConsent::Disabled`] (the
//! default) the returned reporter must accept any event without panicking,
//! without erroring, and without performing observable I/O.

use copypaste_telemetry::{
    init, ErrorReporter, NoopReporter, OsTag, ReportConsent, ReportableError,
};

fn sample_event() -> ReportableError {
    ReportableError::new(
        "copypaste-daemon",
        "0.2.0-beta.0",
        "ipc.parse_error",
        OsTag::current(),
    )
}

#[test]
fn default_consent_yields_noop() {
    let reporter = init(ReportConsent::default());
    // Many calls, all swallowed.
    for _ in 0..1_000 {
        assert!(reporter.report(sample_event()).is_ok());
    }
}

#[test]
fn noop_swallows_every_os_tag() {
    let reporter = NoopReporter::new();
    for os in [OsTag::MacOs, OsTag::Windows, OsTag::Android, OsTag::Unknown] {
        let evt = ReportableError::new("c", "0.0.0", "class", os);
        assert!(reporter.report(evt).is_ok());
    }
}

#[test]
fn noop_handles_empty_strings_without_panic() {
    let reporter = NoopReporter::new();
    let evt = ReportableError::new("", "", "", OsTag::Unknown);
    assert!(reporter.report(evt).is_ok());
}

#[test]
fn noop_is_send_sync_and_threadsafe() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NoopReporter>();

    let reporter: Box<dyn ErrorReporter> = init(ReportConsent::Disabled);
    let reporter = std::sync::Arc::new(reporter);
    let mut handles = Vec::new();
    for _ in 0..8 {
        let r = std::sync::Arc::clone(&reporter);
        handles.push(std::thread::spawn(move || {
            for _ in 0..256 {
                let _ = r.report(sample_event());
            }
        }));
    }
    for h in handles {
        h.join().expect("worker thread panicked");
    }
}
