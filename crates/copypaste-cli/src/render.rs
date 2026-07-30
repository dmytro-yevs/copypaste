//! Human-readable output.
//!
//! Everything here is a pure function of typed [`copypaste_ipc`] values plus an
//! explicit `now`, so the formatting is unit-testable without a daemon and
//! without a wall clock.

use comfy_table::{presets, ContentArrangement, Table};
use copypaste_ipc::{
    CloudStatusData, CloudSyncData, Item, PairingData, PeerInfo, StatusData, SyncResult,
    PROTOCOL_VERSION,
};

/// Stand-in printed instead of a sensitive item's content.
///
/// The list view must never render the plaintext of an item the detector
/// flagged (CLAUDE.md rule 4 / port manifest 07 I9). Spelled the same way the
/// core redactor spells it so the two read as one convention.
pub const REDACTED: &str = "***REDACTED***";

/// Marks a pinned row.
pub const PIN_GLYPH: &str = "*";
/// Marks a sensitive row.
pub const SENSITIVE_GLYPH: &str = "!";

/// How much of an item's content a table row shows.
const CONTENT_WIDTH: usize = 56;

/// Compact "how long ago", e.g. `12m ago`.
///
/// `created_at_ms` and `now_ms` are both milliseconds since the Unix epoch, the
/// unit [`Item::created_at`] uses.
pub fn relative_time(created_at_ms: i64, now_ms: i64) -> String {
    let secs = (now_ms - created_at_ms) / 1000;
    if secs < 0 {
        // Clock skew between the daemon's write and this process' read. Saying
        // "now" is less alarming, and less wrong, than "-3s ago".
        return "now".to_string();
    }
    match secs {
        0..=4 => "now".to_string(),
        5..=59 => format!("{secs}s ago"),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86_399 => format!("{}h ago", secs / 3600),
        86_400..=604_799 => format!("{}d ago", secs / 86_400),
        604_800..=31_535_999 => format!("{}w ago", secs / 604_800),
        _ => format!("{}y ago", secs / 31_536_000),
    }
}

/// Collapse content to a single display line of at most `max` characters.
///
/// Newlines end the line, control characters become spaces, and either kind of
/// shortening is signalled with an ellipsis so a truncated value is never
/// mistaken for the whole value.
pub fn one_line(content: &str, max: usize) -> String {
    let first = content.split(['\n', '\r']).next().unwrap_or("");
    // A trailing newline is not "more content"; anything else after the first
    // line is, and the reader has to be told the value was cut.
    let had_more = !content[first.len()..].trim_matches(['\n', '\r']).is_empty();

    let cleaned: String = first
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let cleaned = cleaned.trim_end();

    if cleaned.chars().count() > max {
        let head: String = cleaned.chars().take(max.saturating_sub(1)).collect();
        return format!("{head}…");
    }
    if had_more && !cleaned.is_empty() {
        return format!("{cleaned}…");
    }
    if cleaned.is_empty() && had_more {
        return "…".to_string();
    }
    cleaned.to_string()
}

/// What a row shows in its content column.
///
/// Sensitive items are redacted here, once, so no caller can forget.
pub fn item_preview(item: &Item, max: usize) -> String {
    if item.is_sensitive {
        return REDACTED.to_string();
    }
    let line = one_line(&item.content, max);
    if line.is_empty() {
        // Non-text items (images, files) may carry no printable content.
        return format!("[{}]", item.content_type);
    }
    line
}

/// Per-row flag glyphs: pinned and/or sensitive.
pub fn item_flags(item: &Item) -> String {
    let mut flags = String::new();
    if item.pinned {
        flags.push_str(PIN_GLYPH);
    }
    if item.is_sensitive {
        flags.push_str(SENSITIVE_GLYPH);
    }
    flags
}

/// Render items as a table, or `empty` when there are none.
pub fn items_table(items: &[Item], now_ms: i64, empty: &str) -> String {
    if items.is_empty() {
        return empty.to_string();
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_HORIZONTAL_ONLY);
    // Disabled, not Dynamic: content is already truncated to a known width, and
    // a fixed arrangement keeps output identical in a pipe and in a terminal.
    table.set_content_arrangement(ContentArrangement::Disabled);
    table.set_header(vec!["ID", "AGE", "FLAGS", "CONTENT"]);

    for item in items {
        table.add_row(vec![
            item.id.clone(),
            relative_time(item.created_at, now_ms),
            item_flags(item),
            item_preview(item, CONTENT_WIDTH),
        ]);
    }

    let mut out = table.to_string();
    let any_pinned = items.iter().any(|i| i.pinned);
    let any_sensitive = items.iter().any(|i| i.is_sensitive);
    if any_pinned || any_sensitive {
        let mut legend = Vec::new();
        if any_pinned {
            legend.push(format!("{PIN_GLYPH} pinned"));
        }
        if any_sensitive {
            legend.push(format!("{SENSITIVE_GLYPH} sensitive (content hidden)"));
        }
        out.push('\n');
        out.push_str(&legend.join("   "));
    }
    out
}

/// Render paired devices as a table, or `empty` when there are none.
///
/// No secret appears here: a peer record holds a pre-shared key, but
/// [`PeerInfo`] does not carry it and the daemon never puts it on the wire. The
/// pairing id is a derived, non-secret identifier and is shown in full because
/// it is what `unpair` and `sync --peer` take.
pub fn peers_table(peers: &[PeerInfo], now_ms: i64, empty: &str) -> String {
    if peers.is_empty() {
        return empty.to_string();
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_HORIZONTAL_ONLY);
    table.set_content_arrangement(ContentArrangement::Disabled);
    table.set_header(vec!["PAIRING ID", "NAME", "SEEN", "ADDRESS", "STATUS"]);

    for peer in peers {
        table.add_row(vec![
            peer.pairing_id.clone(),
            one_line(&peer.name, 24),
            if peer.last_seen_ms > 0 {
                relative_time(peer.last_seen_ms, now_ms)
            } else {
                "never".to_string()
            },
            peer.last_addr.clone().unwrap_or_else(|| "—".to_string()),
            // "offline" is "not seen on the network", never "unreachable":
            // discovery is a convenience and an explicit address always works.
            if peer.online { "online" } else { "offline" }.to_string(),
        ]);
    }
    table.to_string()
}

/// Render one sync run, one row per peer.
pub fn sync_table(results: &[SyncResult], empty: &str) -> String {
    if results.is_empty() {
        return empty.to_string();
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_HORIZONTAL_ONLY);
    table.set_content_arrangement(ContentArrangement::Disabled);
    table.set_header(vec!["PEER", "SENT", "RECEIVED", "RESULT"]);

    for result in results {
        table.add_row(vec![
            one_line(&result.name, 24),
            result.sent.to_string(),
            result.received.to_string(),
            match &result.error {
                // Already a fixed sentence from the daemon; scrubbed anyway,
                // because this CLI is what the user actually reads.
                Some(error) => crate::error::scrub_paths(error),
                None => "ok".to_string(),
            },
        ]);
    }
    table.to_string()
}

/// Render a freshly minted pairing, code and all.
///
/// The code is the one secret this CLI ever prints. It goes to stdout so it can
/// be read out or piped, and the surrounding text says plainly what it is worth
/// — a code in a chat log is a paired device.
pub fn pairing_text(pairing: &PairingData) -> String {
    let mut lines = vec![
        format!("{:<12} {}", "code", pairing.code),
        format!("{:<12} {}", "pairing id", pairing.pairing_id),
    ];
    match &pairing.listen_addr {
        Some(addr) => lines.push(format!("{:<12} {}", "address", addr)),
        None => lines.push(format!(
            "{:<12} {}",
            "address", "unknown — give the other device this host's address"
        )),
    }
    lines.push(String::new());
    lines.push("On the other device, run:".to_string());
    lines.push(format!(
        "  copypaste pair accept {} --addr {}",
        pairing.code,
        pairing.listen_addr.as_deref().unwrap_or("HOST:PORT")
    ));
    lines.push(String::new());
    lines.push(
        "The code is a secret and is shown only once: anyone holding it can pair \
         with this device."
            .to_string(),
    );
    lines.join("\n")
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

    fn item(content: &str) -> Item {
        Item {
            id: "3f2a91c4-0000-4000-8000-000000000001".into(),
            content: content.into(),
            content_type: "text/plain".into(),
            created_at: 1_000_000,
            pinned: false,
            is_sensitive: false,
        }
    }

    const MIN: i64 = 60_000;
    const HOUR: i64 = 60 * MIN;
    const DAY: i64 = 24 * HOUR;

    #[test]
    fn relative_time_buckets() {
        let now = 10 * DAY;
        assert_eq!(relative_time(now, now), "now");
        assert_eq!(relative_time(now - 3_000, now), "now");
        assert_eq!(relative_time(now - 30_000, now), "30s ago");
        assert_eq!(relative_time(now - 5 * MIN, now), "5m ago");
        assert_eq!(relative_time(now - 3 * HOUR, now), "3h ago");
        assert_eq!(relative_time(now - 2 * DAY, now), "2d ago");
        assert_eq!(relative_time(now - 9 * DAY, now), "1w ago");
    }

    #[test]
    fn relative_time_tolerates_a_future_timestamp() {
        assert_eq!(relative_time(2_000, 1_000), "now");
    }

    #[test]
    fn one_line_keeps_short_single_line_content_verbatim() {
        assert_eq!(one_line("hello", 20), "hello");
    }

    #[test]
    fn one_line_stops_at_the_first_newline() {
        assert_eq!(one_line("first\nsecond", 20), "first…");
    }

    #[test]
    fn one_line_truncates_to_max_chars() {
        let out = one_line(&"x".repeat(100), 10);
        assert_eq!(out.chars().count(), 10);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn one_line_truncates_on_char_boundaries() {
        let out = one_line("日本語のテキストがとても長い場合", 5);
        assert_eq!(out, "日本語の…");
    }

    #[test]
    fn one_line_replaces_control_characters() {
        assert_eq!(one_line("a\tb\u{7}c", 20), "a b c");
    }

    #[test]
    fn one_line_ignores_a_trailing_newline_as_content() {
        // A trailing newline alone must not make the value look truncated.
        assert_eq!(one_line("hello\n", 20), "hello");
        assert_eq!(one_line("hello\n\n", 20), "hello");
    }

    #[test]
    fn sensitive_content_is_never_rendered() {
        let mut it = item("sk-live-4eC39HqLyjWDarjtT1zdp7dc");
        it.is_sensitive = true;
        let preview = item_preview(&it, 56);
        assert_eq!(preview, REDACTED);
        assert!(!preview.contains("sk-live"));
    }

    #[test]
    fn sensitive_content_is_absent_from_the_whole_table() {
        let mut it = item("AKIAIOSFODNN7EXAMPLE");
        it.is_sensitive = true;
        let table = items_table(&[it], 1_000_000, "no items");
        assert!(!table.contains("AKIAIOSFODNN7EXAMPLE"), "{table}");
        assert!(table.contains(REDACTED), "{table}");
        assert!(table.contains(SENSITIVE_GLYPH), "{table}");
    }

    #[test]
    fn empty_content_falls_back_to_the_content_type() {
        let mut it = item("");
        it.content_type = "image/png".into();
        assert_eq!(item_preview(&it, 56), "[image/png]");
    }

    #[test]
    fn flags_mark_pinned_and_sensitive() {
        let mut it = item("x");
        assert_eq!(item_flags(&it), "");
        it.pinned = true;
        assert_eq!(item_flags(&it), PIN_GLYPH);
        it.is_sensitive = true;
        assert_eq!(item_flags(&it), format!("{PIN_GLYPH}{SENSITIVE_GLYPH}"));
    }

    #[test]
    fn empty_list_renders_the_empty_message() {
        assert_eq!(items_table(&[], 0, "no items yet"), "no items yet");
    }

    #[test]
    fn table_shows_the_full_id_so_it_can_be_copied_into_another_command() {
        let it = item("hello");
        let table = items_table(std::slice::from_ref(&it), 1_000_000, "none");
        assert!(table.contains(&it.id), "{table}");
    }

    #[test]
    fn table_includes_headers_and_a_relative_age() {
        let mut it = item("hello");
        it.created_at = 0;
        let table = items_table(&[it], 5 * MIN, "none");
        assert!(table.contains("CONTENT"), "{table}");
        assert!(table.contains("5m ago"), "{table}");
    }

    #[test]
    fn table_legend_appears_only_when_a_flag_is_used() {
        let plain = items_table(&[item("hello")], 1_000_000, "none");
        assert!(!plain.contains("pinned"), "{plain}");

        let mut pinned = item("hello");
        pinned.pinned = true;
        let out = items_table(&[pinned], 1_000_000, "none");
        assert!(out.contains("pinned"), "{out}");
    }

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

    fn peer(name: &str) -> PeerInfo {
        PeerInfo {
            pairing_id: "0123456789abcdef0123456789abcdef".into(),
            name: name.into(),
            last_addr: Some("192.168.1.24:47654".into()),
            last_seen_ms: 0,
            online: false,
        }
    }

    #[test]
    fn an_empty_peer_list_renders_the_empty_message() {
        assert_eq!(
            peers_table(&[], 0, "no paired devices"),
            "no paired devices"
        );
    }

    #[test]
    fn peers_table_shows_the_full_pairing_id_so_it_can_be_copied() {
        let p = peer("phone");
        let table = peers_table(std::slice::from_ref(&p), 1_000_000, "none");
        assert!(table.contains(&p.pairing_id), "{table}");
        assert!(table.contains("192.168.1.24:47654"), "{table}");
    }

    #[test]
    fn a_peer_never_seen_says_so_rather_than_showing_the_epoch() {
        let table = peers_table(&[peer("phone")], 1_000_000, "none");
        assert!(table.contains("never"), "{table}");
    }

    #[test]
    fn discovery_state_reads_as_seen_or_not_seen() {
        let mut p = peer("phone");
        assert!(peers_table(std::slice::from_ref(&p), 0, "none").contains("offline"));
        p.online = true;
        p.last_seen_ms = 1_000;
        let table = peers_table(&[p], 31_000, "none");
        assert!(table.contains("online"), "{table}");
        assert!(table.contains("30s ago"), "{table}");
    }

    fn sync_result(error: Option<&str>) -> SyncResult {
        SyncResult {
            pairing_id: "0123456789abcdef0123456789abcdef".into(),
            name: "phone".into(),
            sent: 3,
            received: 2,
            error: error.map(str::to_string),
        }
    }

    #[test]
    fn sync_table_reports_counts_and_failures_side_by_side() {
        let table = sync_table(
            &[sync_result(None), sync_result(Some("unreachable"))],
            "none",
        );
        assert!(table.contains("ok"), "{table}");
        assert!(table.contains("unreachable"), "{table}");
        assert!(table.contains('3') && table.contains('2'), "{table}");
    }

    #[test]
    fn a_daemon_supplied_path_is_scrubbed_out_of_a_sync_failure() {
        let table = sync_table(
            &[sync_result(Some("failed at /Users/dmitriy/Library/x.db"))],
            "none",
        );
        assert!(!table.contains("dmitriy"), "{table}");
    }

    #[test]
    fn an_empty_sync_run_renders_the_empty_message() {
        assert_eq!(sync_table(&[], "no paired devices"), "no paired devices");
    }

    #[test]
    fn a_pairing_shows_the_code_and_says_it_is_a_secret() {
        let text = pairing_text(&PairingData {
            code: "ABCD-EFGH-JKMN".into(),
            pairing_id: "0123456789abcdef".into(),
            listen_addr: Some("192.168.1.24:47654".into()),
        });
        assert!(text.contains("ABCD-EFGH-JKMN"), "{text}");
        assert!(text.contains("secret"), "{text}");
        // The command to run on the other device must be copy-pasteable whole.
        assert!(
            text.contains("copypaste pair accept ABCD-EFGH-JKMN --addr 192.168.1.24:47654"),
            "{text}"
        );
    }

    #[test]
    fn a_pairing_with_no_reachable_address_says_what_to_do() {
        let text = pairing_text(&PairingData {
            code: "ABCD-EFGH".into(),
            pairing_id: "0123456789abcdef".into(),
            listen_addr: None,
        });
        assert!(text.contains("HOST:PORT"), "{text}");
        assert!(text.contains("unknown"), "{text}");
    }

    #[test]
    fn status_never_prints_a_path() {
        let text = status_text(&status(PROTOCOL_VERSION, "macos-pasteboard"));
        assert!(!text.contains('/'), "{text}");
    }
}
