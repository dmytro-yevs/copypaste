//! Human-readable output.
//!
//! Everything here is a pure function of typed [`copypaste_ipc`] values plus an
//! explicit `now`, so the formatting is unit-testable without a daemon and
//! without a wall clock.
//!
//! This file holds the tables — a row per record — and the two string helpers
//! every one of them uses. [`service`] holds the key/value blocks.

mod service;

pub use service::{
    cloud_status_text, cloud_sync_text, config_text, event_text, export_summary, status_text,
};

use comfy_table::{presets, ContentArrangement, Table};
use copypaste_ipc::{DiscoveredDevice, Item, PairingData, PeerInfo, SyncResult};

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
/// Marks a row cloud sync will not carry, because it is over the per-item cap.
pub const TOO_LARGE_GLYPH: &str = "~";

/// How much of an item's content a table row shows.
const CONTENT_WIDTH: usize = 56;
/// How much of a device name a row shows.
const DEVICE_WIDTH: usize = 16;

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
        // A representation with no printable form — an image, a file — says
        // what it is rather than rendering as a blank row.
        return format!(
            "[{}]",
            copypaste_ipc::content_type::label(&item.content_type)
        );
    }
    line
}

/// Per-row flag glyphs: pinned, sensitive, and too large to sync.
pub fn item_flags(item: &Item) -> String {
    let mut flags = String::new();
    if item.pinned {
        flags.push_str(PIN_GLYPH);
    }
    if item.is_sensitive {
        flags.push_str(SENSITIVE_GLYPH);
    }
    if item.too_large_to_sync {
        flags.push_str(TOO_LARGE_GLYPH);
    }
    flags
}

/// Which device an item came from, as a column value.
///
/// The name when this device has been told one, and a short form of the id when
/// it has not. Never blank and never silently "this device": an item whose
/// origin is unknown is a different fact from an item captured here, and
/// collapsing the two is the whole of UI audit finding 3.
pub fn origin_label(item: &Item) -> String {
    match &item.origin_device_name {
        Some(name) => one_line(name, DEVICE_WIDTH),
        None if item.origin_device_id.is_empty() => "unknown".to_string(),
        // A UUID's first group is enough to tell two devices apart by eye, and
        // the full id is on the wire for anything that needs to match on it.
        None => item.origin_device_id.chars().take(8).collect(),
    }
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
    table.set_header(vec!["ID", "AGE", "FROM", "FLAGS", "CONTENT"]);

    for item in items {
        table.add_row(vec![
            item.id.clone(),
            relative_time(item.created_at, now_ms),
            origin_label(item),
            item_flags(item),
            item_preview(item, CONTENT_WIDTH),
        ]);
    }

    let mut out = table.to_string();
    let mut legend = Vec::new();
    if items.iter().any(|i| i.pinned) {
        legend.push(format!("{PIN_GLYPH} pinned"));
    }
    if items.iter().any(|i| i.is_sensitive) {
        legend.push(format!("{SENSITIVE_GLYPH} sensitive (content hidden)"));
    }
    if items.iter().any(|i| i.too_large_to_sync) {
        legend.push(format!("{TOO_LARGE_GLYPH} too large to sync"));
    }
    if !legend.is_empty() {
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

/// Render LAN devices, or `empty` when none are visible.
///
/// "None visible" is a normal answer, not a failure: discovery may be switched
/// off and multicast is filtered on plenty of networks. The empty message says
/// what to do instead rather than implying something is broken.
pub fn discovered_table(devices: &[DiscoveredDevice], now_ms: i64, empty: &str) -> String {
    if devices.is_empty() {
        return format!(
            "{empty}\n\nPair with an address instead: copypaste pair create, then \
             copypaste pair accept CODE --addr HOST:PORT on the other device."
        );
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_HORIZONTAL_ONLY);
    table.set_content_arrangement(ContentArrangement::Disabled);
    table.set_header(vec!["DISCOVERY ID", "NAME", "ADDRESS", "SEEN", "STATUS"]);

    for device in devices {
        table.add_row(vec![
            device.discovery_id.clone(),
            // Advertised by whoever is on the network, so it is truncated and
            // stripped like any other untrusted string and never used as an
            // identity.
            one_line(&device.name, 24),
            device.addr.clone(),
            relative_time(device.last_seen_ms, now_ms),
            if device.paired { "paired" } else { "new" }.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn item(content: &str) -> Item {
        Item {
            id: "3f2a91c4-0000-4000-8000-000000000001".into(),
            content: content.into(),
            content_type: "text".into(),
            created_at: 1_000_000,
            pinned: false,
            is_sensitive: false,
            origin_device_id: "9e1d0000-0000-4000-8000-00000000000a".into(),
            origin_device_name: Some("This Mac".into()),
            source_app_bundle_id: None,
            source_app_name: None,
            too_large_to_sync: false,
            truncated: false,
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
    fn a_representation_with_no_text_says_what_it_is() {
        let mut it = item("");
        it.content_type = "image/png".into();
        assert_eq!(item_preview(&it, 56), "[image]");
        it.content_type = "application/x-not-invented-yet".into();
        assert_eq!(item_preview(&it, 56), "[unsupported]");
    }

    #[test]
    fn flags_mark_pinned_sensitive_and_unsyncable() {
        let mut it = item("x");
        assert_eq!(item_flags(&it), "");
        it.pinned = true;
        assert_eq!(item_flags(&it), PIN_GLYPH);
        it.is_sensitive = true;
        assert_eq!(item_flags(&it), format!("{PIN_GLYPH}{SENSITIVE_GLYPH}"));
        it.too_large_to_sync = true;
        assert_eq!(
            item_flags(&it),
            format!("{PIN_GLYPH}{SENSITIVE_GLYPH}{TOO_LARGE_GLYPH}")
        );
    }

    /// An item that will never reach the other device must not look like one
    /// that is still on its way (`CopyPaste-f72f`).
    #[test]
    fn an_unsyncable_item_is_marked_and_explained() {
        let mut it = item("a very large clip");
        it.too_large_to_sync = true;
        let table = items_table(&[it], 1_000_000, "none");
        assert!(table.contains(TOO_LARGE_GLYPH), "{table}");
        assert!(table.contains("too large to sync"), "{table}");
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

    /// UI audit finding 3, in its command-line form: with sync on, a row from
    /// the phone must not read as local.
    #[test]
    fn a_row_says_which_device_it_came_from() {
        let table = items_table(&[item("hello")], 1_000_000, "none");
        assert!(table.contains("This Mac"), "{table}");

        let mut theirs = item("from the phone");
        theirs.origin_device_name = Some("Phone".into());
        let table = items_table(&[theirs], 1_000_000, "none");
        assert!(table.contains("Phone"), "{table}");
    }

    /// A device this one has never spoken to has an id and no name. Showing the
    /// id is honest; showing this device's name would not be.
    #[test]
    fn an_unnamed_origin_falls_back_to_a_short_id_never_to_this_device() {
        let mut it = item("x");
        it.origin_device_name = None;
        assert_eq!(origin_label(&it), "9e1d0000");

        it.origin_device_id = String::new();
        assert_eq!(origin_label(&it), "unknown");
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
            error_code: None,
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
}
