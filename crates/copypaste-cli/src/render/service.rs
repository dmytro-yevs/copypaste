//! Rendering for everything that is not a list of things: the daemon's state,
//! its settings, a cloud round, one change event, and what an export left out.
//!
//! Split from the tables in [`super`] on the same line the output itself
//! divides: a table is a row per record and reads left to right, a block is a
//! label per line and reads top to bottom.

use copypaste_ipc::{
    CloudStatusData, CloudSyncData, ConfigApplied, EventData, EventKind, ExportData, StatusData,
    PROTOCOL_VERSION,
};

use super::relative_time;

/// What an export left behind, for stderr.
///
/// Every count is printed, including zero, for the same reason `withheld` is in
/// [`cloud_sync_text`]: a number that only appears when it is non-zero is one
/// nobody knows to look for.
pub fn export_summary(export: &ExportData) -> String {
    let mut lines = vec![format!(
        "exported {} items; withheld {} sensitive, skipped {} non-text, {} unreadable",
        export.items.len(),
        export.skipped_sensitive,
        export.skipped_non_text,
        export.skipped_undecryptable,
    )];
    if export.skipped_sensitive > 0 {
        lines.push(
            "Sensitive items are withheld by default. Pass --include-sensitive to \
             include them — the export is a plaintext file."
                .to_string(),
        );
    }
    lines.push(String::new());
    lines.join("\n")
}

/// Render the daemon's settings, and anything that needs a restart.
pub fn config_text(applied: &ConfigApplied) -> String {
    let config = &applied.config;
    let mut lines = vec![
        setting("poll interval", format!("{} ms", config.poll_interval_ms)),
        setting("history limit", format!("{} items", config.history_limit)),
        setting(
            "retention",
            match config.retention_days {
                0 => "off".to_string(),
                days => format!("{days} days"),
            },
        ),
        setting("dedup window", format!("{} s", config.dedup_window_secs)),
        setting("max item size", format!("{} bytes", config.max_item_bytes)),
        setting(
            "sensitive ttl",
            match config.sensitive_ttl_secs {
                // `0` is "never delete", not "delete immediately". Rendering it
                // as a number would read as the opposite of what it means.
                0 => "off".to_string(),
                secs => format!("{secs} s"),
            },
        ),
        setting(
            "excluded apps",
            if config.excluded_app_bundle_ids.is_empty() {
                "none".to_string()
            } else {
                config.excluded_app_bundle_ids.join(", ")
            },
        ),
        setting("lan visibility", yes_no(config.lan_visibility)),
        setting("sync", yes_no(config.sync_enabled)),
        setting("notify on copy", yes_no(config.notify_on_copy)),
        setting("sound on copy", yes_no(config.sound_on_copy)),
    ];

    if !applied.restart_required.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "changed, but not yet in effect: {}. Restart the daemon to apply.",
            applied.restart_required.join(", ")
        ));
    }
    lines.join("\n")
}

fn setting(name: &str, value: String) -> String {
    format!("{name:<16} {value}")
}

fn yes_no(value: bool) -> String {
    if value { "on" } else { "off" }.to_string()
}

/// One line per change event, for `copypaste watch`.
pub fn event_text(event: &EventData) -> String {
    match event.event {
        // The capture case is called out because it is the one a person
        // watching the stream is usually waiting for, and because it is what
        // the notification and the sound are gated on (parity finding 18).
        EventKind::Items if event.captured => {
            format!("captured an item ({} in history)", event.item_count)
        }
        EventKind::Items => format!("items changed ({} in history)", event.item_count),
        EventKind::Peers => "paired devices changed".to_string(),
    }
}

/// Render `status` as an aligned key/value block.
pub fn status_text(status: &StatusData) -> String {
    let mut lines = vec![
        format!("{:<12} {}", "daemon", "running"),
        format!("{:<12} {}", "version", status.version),
        format!(
            "{:<12} {} (this CLI: {})",
            "protocol", status.protocol_version, PROTOCOL_VERSION
        ),
        format!("{:<12} {}", "items", status.item_count),
        format!(
            "{:<12} {}",
            "capture",
            if status.capture_running {
                "running"
            } else {
                "stopped"
            }
        ),
        format!("{:<12} {}", "clipboard", clipboard_backend(status)),
    ];

    if status.protocol_version != PROTOCOL_VERSION {
        // Manifest 04 §6.2: a client must not silently continue across a
        // protocol difference.
        lines.push(String::new());
        lines.push(
            "warning: the daemon and this CLI speak different IPC protocol versions. \
             Upgrade both to the same release and restart the daemon."
                .to_string(),
        );
    }

    lines.join("\n")
}

/// Render cloud sync status as an aligned key/value block.
///
/// Names no URL and no token: `configured` is the only thing a user needs to
/// know about the deployment, and printing the project URL puts it in every
/// screenshot of a bug report.
pub fn cloud_status_text(status: &CloudStatusData, now_ms: i64) -> String {
    if !status.configured {
        return "cloud sync is not configured on this daemon\n\n\
                Start the daemon with --cloud-url and --cloud-anon-key (or set \
                COPYPASTE_CLOUD_URL and COPYPASTE_CLOUD_ANON_KEY) to enable it."
            .to_string();
    }

    let mut lines = vec![
        format!("{:<12} {}", "configured", "yes"),
        format!(
            "{:<12} {}",
            "account",
            status.email.as_deref().unwrap_or("signed out")
        ),
        format!(
            "{:<12} {}",
            "sync key",
            if status.key_ready {
                "unlocked"
            } else {
                "locked — sign in to derive it"
            }
        ),
        format!(
            "{:<12} {}",
            "last sync",
            match status.last_sync_ms {
                Some(ms) => relative_time(ms, now_ms),
                None => "never".to_string(),
            }
        ),
        format!("{:<12} {}s", "polling", status.poll_interval_secs),
    ];
    if let Some(error) = &status.last_error {
        lines.push(format!("{:<12} {}", "last error", error));
    }
    if !status.signed_in {
        lines.push(String::new());
        lines.push("Sign in with: copypaste cloud sign-in --email you@example.com".to_string());
    }
    lines.join("\n")
}

/// Render one cloud round.
///
/// `withheld` is always printed, including zero: it is the line a user checks
/// the "a sensitive item is never uploaded" rule against, and a count that only
/// appears when it is non-zero is one nobody knows to look for.
pub fn cloud_sync_text(stats: &CloudSyncData) -> String {
    let mut lines = vec![
        format!("{:<12} {}", "uploaded", stats.uploaded),
        format!("{:<12} {}", "deleted", stats.tombstoned),
        format!("{:<12} {}", "downloaded", stats.downloaded),
        format!("{:<12} {}", "applied", stats.applied),
        format!(
            "{:<12} {} (sensitive, never uploaded)",
            "withheld", stats.skipped_sensitive
        ),
    ];
    if stats.skipped_undecryptable > 0 {
        lines.push(format!(
            "{:<12} {} (not readable with this sync passphrase)",
            "skipped", stats.skipped_undecryptable
        ));
    }
    if stats.skipped_future > 0 {
        lines.push(format!(
            "{:<12} {} (stamped implausibly far in the future)",
            "refused", stats.skipped_future
        ));
    }
    if stats.skipped_too_large > 0 {
        // Withheld, not lost: the items are still on this device in full. The
        // sentence says so, because "skipped" alone reads like a deletion.
        lines.push(format!(
            "{:<12} {} (too large to sync; kept on this device)",
            "withheld", stats.skipped_too_large
        ));
    }
    lines.join("\n")
}

/// The backend string, annotated when it is not the real system clipboard.
///
/// `StatusData::clipboard_backend` exists so a demo cannot be mistaken for the
/// real thing; passing it through unannotated would defeat that.
fn clipboard_backend(status: &StatusData) -> String {
    let backend = &status.clipboard_backend;
    let lowered = backend.to_ascii_lowercase();
    if lowered.contains("fake") || lowered.contains("mock") || lowered.contains("null") {
        format!("{backend} (not the system clipboard)")
    } else {
        backend.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(protocol_version: u32, backend: &str) -> StatusData {
        StatusData {
            version: "2.0.0-alpha.1".into(),
            protocol_version,
            item_count: 42,
            capture_running: true,
            clipboard_backend: backend.into(),
        }
    }

    #[test]
    fn status_reports_the_essentials() {
        let text = status_text(&status(PROTOCOL_VERSION, "macos-pasteboard"));
        assert!(text.contains("2.0.0-alpha.1"), "{text}");
        assert!(text.contains("42"), "{text}");
        assert!(text.contains("running"), "{text}");
        assert!(!text.contains("warning"), "{text}");
    }

    #[test]
    fn status_warns_on_a_protocol_difference() {
        let text = status_text(&status(PROTOCOL_VERSION + 1, "macos-pasteboard"));
        assert!(text.contains("warning"), "{text}");
        assert!(text.contains("different IPC protocol versions"), "{text}");
    }

    #[test]
    fn status_flags_a_fake_clipboard_backend() {
        let text = status_text(&status(PROTOCOL_VERSION, "fake"));
        assert!(text.contains("not the system clipboard"), "{text}");
    }

    #[test]
    fn status_never_prints_a_path() {
        let text = status_text(&status(PROTOCOL_VERSION, "macos-pasteboard"));
        assert!(!text.contains('/'), "{text}");
    }

    /// Every setting the daemon has must be printable, or `config show` is a
    /// list a user cannot trust to be complete.
    #[test]
    fn config_shows_the_two_copy_feedback_switches() {
        let applied = ConfigApplied {
            config: copypaste_ipc::ConfigData {
                notify_on_copy: true,
                ..Default::default()
            },
            restart_required: Vec::new(),
        };
        let text = config_text(&applied);
        assert!(text.contains("notify on copy   on"), "{text}");
        assert!(text.contains("sound on copy    off"), "{text}");
    }

    fn stats() -> CloudSyncData {
        CloudSyncData {
            uploaded: 0,
            tombstoned: 0,
            downloaded: 0,
            applied: 0,
            skipped_sensitive: 0,
            skipped_undecryptable: 0,
            skipped_future: 0,
            skipped_too_large: 0,
        }
    }

    /// An item over the cap will never reach the account. A round that says
    /// nothing about it is indistinguishable from one that uploaded everything.
    #[test]
    fn a_round_says_when_something_was_too_large_to_carry() {
        let quiet = cloud_sync_text(&stats());
        assert!(!quiet.contains("too large"), "{quiet}");

        let loud = cloud_sync_text(&CloudSyncData {
            skipped_too_large: 2,
            ..stats()
        });
        assert!(loud.contains("too large to sync"), "{loud}");
        assert!(
            loud.contains("kept on this device"),
            "withholding must not read as deletion: {loud}"
        );
    }

    #[test]
    fn a_capture_event_reads_differently_from_any_other_change() {
        let captured = event_text(&EventData {
            event: EventKind::Items,
            item_count: 3,
            captured: true,
        });
        assert!(captured.contains("captured"), "{captured}");

        let other = event_text(&EventData {
            event: EventKind::Items,
            item_count: 3,
            captured: false,
        });
        assert!(!other.contains("captured"), "{other}");
    }
}
