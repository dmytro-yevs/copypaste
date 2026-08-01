//! What is running, what has been dropped, and one block of text to paste into
//! an issue.
//!
//! Deliberately not a log viewer: streaming the daemon's stderr needs a wire
//! mechanism, a retention policy and a redaction pass on every line forever,
//! and the line that slips through is the one with a clipping in it.
//!
//! No path — [`report`] is assembled here, not in the frontend catalogue, so
//! it goes through the one redactor first. No clipboard content: nothing in
//! [`Diagnostics`] can carry any, because there is no field one could travel
//! in. The shape enforces it; the tests only prove it did not regress.

use copypaste_ipc::{redact::scrub_paths, StatusData, PROTOCOL_VERSION};
use serde::Serialize;

use super::ServiceState;

/// One snapshot, structured for the panel and pre-rendered for the clipboard.
///
/// The report travels with the data it was built from so the two cannot
/// disagree: a user who reads a number on screen and pastes a block underneath
/// is entitled to the same round.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostics {
    pub app_version: String,
    pub app_protocol_version: u32,
    pub os: String,
    pub arch: String,
    pub service: ServiceState,
    /// `None` when the service did not answer — which is itself the answer, and
    /// why the panel must not require it.
    pub status: Option<StatusData>,
    /// Ready to paste. Redacted; never rebuilt in the frontend.
    pub report: String,
}

impl Diagnostics {
    /// Scrubs on the way in, not on the way out.
    ///
    /// The panel renders `clipboard_backend` and the service's version as the
    /// daemon reported them, so redacting only [`Diagnostics::report`] would
    /// leave a path on the screen and in the screenshot attached to the same
    /// issue. Both free-text fields go through the one redactor here, which is
    /// also why nothing downstream needs to remember to.
    pub fn new(service: ServiceState, status: Option<StatusData>) -> Self {
        let service = scrub_service(service);
        let status = status.map(|mut status| {
            status.version = safe_value(&status.version);
            status.clipboard_backend = safe_value(&status.clipboard_backend);
            status
        });
        let report = report(&service, status.as_ref());
        Self {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            app_protocol_version: PROTOCOL_VERSION,
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            service,
            status,
            report,
        }
    }
}

fn scrub_service(service: ServiceState) -> ServiceState {
    match service {
        ServiceState::Running {
            version,
            matches_app,
            ours,
        } => ServiceState::Running {
            version: safe_value(&version),
            matches_app,
            ours,
        },
        other => other,
    }
}

/// Stands in for a whole field that was found to contain a path.
const REDACTED: &str = "<redacted>";

/// Drop the **whole value** when any part of it looked like a path.
///
/// `scrub_paths` is the one redactor and stays the one detector, but it works
/// token by token: it replaces the token that starts with `/` and leaves what
/// follows a space. The real macOS socket path is
/// `~/Library/Application Support/CopyPaste/daemon.sock`, so token-wise
/// redaction keeps the username out and still leaves a recognisable tail — in
/// a block written to be pasted into a public issue.
///
/// A diagnostics value is a version, a backend name or a number. None of them
/// has any business containing a path, so finding one is reason enough to
/// distrust the field rather than to salvage part of it.
fn safe_value(value: &str) -> String {
    if scrub_paths(value) == value {
        value.to_string()
    } else {
        REDACTED.to_string()
    }
}

/// The copyable block: one `label: value` per line.
///
/// Values reach it already through [`safe_value`]; the closing `scrub_paths` is
/// the net for a label or a literal, not the primary defence.
///
/// Not prose and not translated. It is a dump for whoever is helping, in the
/// same category as a version string, and it has to be built where the redactor
/// is.
fn report(service: &ServiceState, status: Option<&StatusData>) -> String {
    let mut out = String::from("CopyPaste diagnostics\n");
    line(&mut out, "app", env!("CARGO_PKG_VERSION"));
    line(
        &mut out,
        "platform",
        &format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
    );
    line(&mut out, "service", &service_line(service));

    let Some(status) = status else {
        // Stated rather than omitted: a report that simply stops is
        // indistinguishable from one that was truncated on the way to the
        // issue.
        line(&mut out, "status", "not answering");
        return scrub_paths(&out);
    };

    line(
        &mut out,
        "protocol",
        &format!("{} (app {})", status.protocol_version, PROTOCOL_VERSION),
    );
    line(
        &mut out,
        "capture",
        if status.capture_running {
            "running"
        } else {
            "stopped"
        },
    );
    line(&mut out, "backend", &status.clipboard_backend);
    line(&mut out, "items", &status.item_count.to_string());
    if status.legacy_history_present {
        line(&mut out, "legacy-history", "present, unopened");
    }

    let c = &status.counters;
    line(&mut out, "uptime-secs", &c.uptime_secs.to_string());
    line(
        &mut out,
        "refused-too-large",
        &c.rejected_too_large.to_string(),
    );
    line(
        &mut out,
        "missed-changes",
        &c.lost_intermediates.to_string(),
    );
    line(
        &mut out,
        "secrets-auto-deleted",
        &c.sensitive_swept.to_string(),
    );
    line(&mut out, "index-rows-purged", &c.index_purged.to_string());

    scrub_paths(&out)
}

fn line(out: &mut String, label: &str, value: &str) {
    out.push_str(label);
    out.push_str(": ");
    out.push_str(value);
    out.push('\n');
}

fn service_line(service: &ServiceState) -> String {
    match service {
        ServiceState::Running {
            version,
            matches_app,
            ours,
        } => format!(
            "running {version}{}{}",
            if *matches_app {
                ""
            } else {
                " (different version to the app)"
            },
            if *ours { "" } else { " (adopted)" },
        ),
        ServiceState::Unhealthy => "answering, but not readable by this build".to_string(),
        ServiceState::Stopped => "stopped".to_string(),
        ServiceState::NotInstalled => "not installed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_ipc::DiagnosticCounters;

    /// Every free-text field a hostile or broken daemon could fill, filled with
    /// the thing rule 4 exists to keep out. The socket path is the real one:
    /// it lives under the home directory, so it spells the local username.
    const LEAKY: &str = "/Users/dmytro/Library/Application Support/CopyPaste/daemon.sock";
    /// Stands in for a clipping. Nothing may put one in a report, so nothing
    /// may put this one in either.
    const SECRET: &str = "sk-live-4f9c1d2e-THIS-IS-A-CLIPPING";

    fn status() -> StatusData {
        StatusData {
            version: "2.0.0-alpha.1".into(),
            protocol_version: PROTOCOL_VERSION,
            item_count: 42,
            capture_running: true,
            clipboard_backend: "macos-pasteboard".into(),
            private_mode: false,
            legacy_history_present: true,
            counters: DiagnosticCounters {
                rejected_too_large: 3,
                lost_intermediates: 1,
                sensitive_swept: 2,
                index_purged: 5,
                uptime_secs: 12_045,
            },
        }
    }

    fn running() -> ServiceState {
        ServiceState::Running {
            version: "2.0.0-alpha.1".into(),
            matches_app: true,
            ours: true,
        }
    }

    /// Always through [`Diagnostics::new`], never the private builder: the
    /// sanitising step is part of what is under test.
    fn report_of(service: ServiceState, status: Option<StatusData>) -> String {
        Diagnostics::new(service, status).report
    }

    #[test]
    fn the_report_states_what_is_running_and_what_was_dropped() {
        let text = report_of(running(), Some(status()));
        for expected in [
            "service: running",
            "capture: running",
            "backend: macos-pasteboard",
            "items: 42",
            "refused-too-large: 3",
            "missed-changes: 1",
            "secrets-auto-deleted: 2",
            "index-rows-purged: 5",
            "legacy-history: present",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
        }
    }

    /// **The test that matters.** A report exists to be pasted into a public
    /// issue, so a path in it is a username in it.
    #[test]
    fn no_path_survives_into_the_report() {
        let mut leaky = status();
        leaky.version = LEAKY.into();
        leaky.clipboard_backend = LEAKY.into();
        let service = ServiceState::Running {
            version: LEAKY.into(),
            matches_app: false,
            ours: false,
        };

        let text = report_of(service, Some(leaky));
        assert!(!text.contains("dmytro"), "username leaked:\n{text}");
        assert!(!text.contains(".sock"), "socket path leaked:\n{text}");
        assert!(!text.contains("/Users/"), "path leaked:\n{text}");
        assert!(!text.contains("/Library/"), "path leaked:\n{text}");
        assert!(!text.contains("Support/"), "path tail leaked:\n{text}");
    }

    /// The **whole payload**, not only the report: the panel renders the
    /// structured half verbatim, and a screenshot of it goes into the same
    /// issue as the paste.
    #[test]
    fn no_path_survives_anywhere_on_the_payload() {
        let mut leaky = status();
        leaky.version = LEAKY.into();
        leaky.clipboard_backend = LEAKY.into();
        let service = ServiceState::Running {
            version: LEAKY.into(),
            matches_app: true,
            ours: true,
        };

        let json = serde_json::to_string(&Diagnostics::new(service, Some(leaky))).unwrap();
        assert!(!json.contains("dmytro"), "username leaked:\n{json}");
        assert!(!json.contains(".sock"), "socket path leaked:\n{json}");
        assert!(!json.contains("/Users/"), "path leaked:\n{json}");
    }

    /// **The other test that matters.** The report has a *closed vocabulary*:
    /// every line is one of the labels above. Clipboard content cannot appear
    /// because there is no line for it to appear on, and adding one is what
    /// this fails on.
    #[test]
    fn the_report_has_a_closed_set_of_lines() {
        const LABELS: &[&str] = &[
            "app",
            "platform",
            "service",
            "status",
            "protocol",
            "capture",
            "backend",
            "items",
            "legacy-history",
            "uptime-secs",
            "refused-too-large",
            "missed-changes",
            "secrets-auto-deleted",
            "index-rows-purged",
        ];
        for (service, status) in [
            (running(), Some(status())),
            (ServiceState::Stopped, None),
            (ServiceState::Unhealthy, None),
            (ServiceState::NotInstalled, None),
        ] {
            let text = report_of(service, status);
            for row in text.lines().skip(1) {
                let label = row.split_once(':').map(|(l, _)| l).unwrap_or(row);
                assert!(LABELS.contains(&label), "unknown line {row:?} in:\n{text}");
            }
        }
    }

    /// A clipping placed where a daemon could put one reaches no line but the
    /// one it was placed on — it is never appended, prefixed or echoed
    /// elsewhere. Together with the closed vocabulary above, that is what makes
    /// "no clip content" structural rather than a habit.
    #[test]
    fn a_clipping_shaped_value_spreads_no_further_than_its_own_field() {
        let mut leaky = status();
        leaky.version = SECRET.into();
        let text = report_of(running(), Some(leaky.clone()));
        assert!(
            !text.contains(SECRET),
            "a field the report does not print appeared in it:\n{text}"
        );

        leaky.clipboard_backend = SECRET.into();
        let text = report_of(running(), Some(leaky));
        assert_eq!(
            text.lines().filter(|l| l.contains(SECRET)).count(),
            1,
            "spread beyond the backend line:\n{text}"
        );
    }

    #[test]
    fn a_service_that_is_not_answering_still_produces_a_report() {
        let text = report_of(ServiceState::Stopped, None);
        assert!(text.contains("service: stopped"), "{text}");
        assert!(text.contains("status: not answering"), "{text}");
        assert!(text.contains("app: "), "{text}");
    }

    #[test]
    fn an_adopted_service_on_another_version_says_both() {
        let text = report_of(
            ServiceState::Running {
                version: "1.9.0".into(),
                matches_app: false,
                ours: false,
            },
            None,
        );
        assert!(text.contains("different version"), "{text}");
        assert!(text.contains("adopted"), "{text}");
    }
}
