use copypaste_ipc::{ExportData, ExportItem};
use tracing::{info, warn};

use crate::storage::{Store, StoreError};
use crate::Keyring;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("the history database could not be accessed")]
    Store(#[from] StoreError),
    #[error("an authenticated text item is too large to export")]
    ContentTooLarge,
}

/// How many rows one export reads out of the store at a time.
const EXPORT_CHUNK: u32 = 500;

/// Read history out as plaintext, counting everything it declines to include.
///
/// `limit` of 0 means everything. `include_sensitive` is an opt-in the wire
/// defaults to off: an export is plaintext and leaves the app's control the
/// moment it is written.
pub fn export(
    store: &Store,
    keyring: &Keyring,
    limit: u32,
    include_sensitive: bool,
) -> Result<ExportData, ExportError> {
    let key = keyring.item_key();
    let mut data = ExportData {
        items: Vec::new(),
        skipped_non_text: 0,
        skipped_sensitive: 0,
        skipped_undecryptable: 0,
    };

    // Paged rather than one unbounded `list`: a full history is 10 000 rows of
    // plaintext and this holds one page of ciphertext at a time on the way to
    // holding all of the plaintext anyway — but the query stays cheap and the
    // limit stops early.
    let mut offset = 0u32;
    loop {
        let rows = store.list(EXPORT_CHUNK, offset)?;
        if rows.is_empty() {
            break;
        }
        offset = offset.saturating_add(EXPORT_CHUNK);

        for row in rows {
            if row.is_sensitive && !include_sensitive {
                data.skipped_sensitive += 1;
                continue;
            }
            if !copypaste_ipc::content_type::is_text(&row.content_type) {
                data.skipped_non_text += 1;
                continue;
            }
            let plaintext = match crate::decrypt(&row.content_ciphertext, &row.nonce, &key, &row.id)
            {
                Ok(plaintext) => plaintext,
                Err(e) => {
                    warn!(id = %row.id, error = ?e, "export skipped an item that failed to decrypt");
                    data.skipped_undecryptable += 1;
                    continue;
                }
            };
            if plaintext.len() > copypaste_ipc::MAX_CONTENT_BYTES {
                return Err(ExportError::ContentTooLarge);
            }
            data.items.push(ExportItem {
                content: String::from_utf8_lossy(&plaintext).into_owned(),
                content_type: row.content_type,
                created_at: row.created_at,
                pinned: row.pinned,
                is_sensitive: row.is_sensitive,
            });
            if limit > 0 && data.items.len() as u32 >= limit {
                return Ok(finish(data));
            }
        }
    }

    Ok(finish(data))
}

fn finish(data: ExportData) -> ExportData {
    // The count only — never content, and never a sample.
    info!(
        items = data.items.len(),
        skipped_sensitive = data.skipped_sensitive,
        skipped_non_text = data.skipped_non_text,
        skipped_undecryptable = data.skipped_undecryptable,
        "exported history"
    );
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::testkit::{fixture, Fixture};

    fn seed_legacy_text(f: &Fixture, content: &[u8], content_type: &str, sensitive: bool) {
        let id = format!("legacy-{}", content.len());
        let key = f.keyring.item_key();
        let (nonce, content_ciphertext) = crate::encrypt(content, &key, &id).unwrap();
        f.store
            .insert(crate::NewItem {
                id,
                content_ciphertext,
                nonce,
                content_type: content_type.to_string(),
                content_hash: crate::compute_content_hash(content),
                is_sensitive: sensitive,
                search_text: (!sensitive).then(|| String::from_utf8_lossy(content).into_owned()),
                created_at: crate::now_ms(),
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .unwrap();
    }

    fn export_of(f: &Fixture, include_sensitive: bool) -> ExportData {
        export(&f.store, &f.keyring, 0, include_sensitive).expect("the store must read")
    }

    #[test]
    fn an_export_carries_the_plaintext_its_stamp_and_its_pin() {
        let f = fixture();
        let id = f.add("first");
        f.store.set_pinned(&id, true).unwrap();
        let created_at = f.store.get(&id).unwrap().unwrap().created_at;
        f.add("second");

        let data = export_of(&f, false);
        assert_eq!(data.items.len(), 2);
        let first = data
            .items
            .iter()
            .find(|i| i.content == "first")
            .expect("the pinned item");
        assert_eq!(first.created_at, created_at);
        assert!(first.pinned);
    }

    /// The property the whole module is shaped around: nothing is dropped
    /// silently.
    #[test]
    fn a_sensitive_item_is_withheld_by_default_and_counted() {
        let f = fixture();
        f.add("ordinary");
        f.add("AKIAIOSFODNN7EXAMPLE");

        let default = export_of(&f, false);
        assert_eq!(default.items.len(), 1);
        assert_eq!(default.skipped_sensitive, 1);
        let rendered = serde_json::to_string(&default).unwrap();
        assert!(
            !rendered.contains("AKIAIOSFODNN7EXAMPLE"),
            "a withheld item's content reached the export"
        );

        // ...and it is present only when asked for explicitly.
        let opted_in = export_of(&f, true);
        assert_eq!(opted_in.items.len(), 2);
        assert_eq!(opted_in.skipped_sensitive, 0);
    }

    /// Every count is on the wire even when it is zero: a field that appears
    /// only when non-zero is one nobody knows to look for.
    #[test]
    fn the_skip_counts_are_always_present() {
        let f = fixture();
        f.add("ordinary");
        let json = serde_json::to_value(export_of(&f, false)).unwrap();
        for field in [
            "skipped_non_text",
            "skipped_sensitive",
            "skipped_undecryptable",
        ] {
            assert_eq!(json[field], 0, "{field} missing or wrong");
        }
    }

    /// Binary payloads are not representable in this plaintext export format.
    /// It is skipped and *counted*, never dropped quietly.
    #[test]
    fn a_non_text_item_is_counted_rather_than_dropped() {
        let f = fixture();
        f.add("ordinary");
        f.add_typed("\u{fffd}PNG", "image/png");

        let data = export_of(&f, false);
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.skipped_non_text, 1);
    }

    /// A row this device's key cannot open is counted too — the third reason an
    /// export can be shorter than the history it came from.
    #[test]
    fn an_unreadable_row_is_counted_rather_than_failing_the_export() {
        let f = fixture();
        f.add("readable");
        let stranger = crate::Keyring::from_secret(&[9u8; 32]);

        let data = export(&f.store, &stranger, 0, false).expect("the store still reads");
        assert!(data.items.is_empty());
        assert_eq!(data.skipped_undecryptable, 1);
    }

    /// The limit stops the read early rather than trimming afterwards, so a
    /// capped export still holds only what it returns.
    #[test]
    fn a_limit_caps_the_items_returned() {
        let f = fixture();
        for n in 0..5 {
            f.add(&format!("item-{n}"));
        }
        assert_eq!(
            export(&f.store, &f.keyring, 2, false).unwrap().items.len(),
            2
        );
        assert_eq!(
            export(&f.store, &f.keyring, 0, false).unwrap().items.len(),
            5
        );
    }

    #[test]
    fn an_authenticated_legacy_text_body_over_the_limit_refuses_the_whole_export() {
        let f = fixture();
        let ordinary = f.add("ordinary");
        f.store.set_pinned(&ordinary, true).unwrap();
        seed_legacy_text(
            &f,
            "\u{1}"
                .repeat(copypaste_ipc::MAX_CONTENT_BYTES + 1)
                .as_bytes(),
            copypaste_ipc::content_type::TEXT,
            false,
        );
        let before = f
            .store
            .list(10, 0)
            .unwrap()
            .into_iter()
            .find(|row| row.id != ordinary)
            .unwrap()
            .content_ciphertext;

        assert!(matches!(
            export(&f.store, &f.keyring, 0, false),
            Err(ExportError::ContentTooLarge)
        ));
        assert_eq!(
            f.store
                .list(10, 0)
                .unwrap()
                .into_iter()
                .find(|row| row.id != ordinary)
                .unwrap()
                .content_ciphertext,
            before
        );
    }

    #[test]
    fn a_sensitive_legacy_body_is_skipped_before_it_is_opened() {
        let f = fixture();
        seed_legacy_text(
            &f,
            "\u{1}"
                .repeat(copypaste_ipc::MAX_CONTENT_BYTES + 1)
                .as_bytes(),
            copypaste_ipc::content_type::TEXT,
            true,
        );
        let data = export_of(&f, false);
        assert_eq!(data.skipped_sensitive, 1);
        assert!(matches!(
            export(&f.store, &f.keyring, 0, true),
            Err(ExportError::ContentTooLarge)
        ));
    }

    #[test]
    fn an_unreadable_oversized_legacy_row_is_still_counted_not_refused_for_size() {
        let f = fixture();
        seed_legacy_text(
            &f,
            "\u{1}"
                .repeat(copypaste_ipc::MAX_CONTENT_BYTES + 1)
                .as_bytes(),
            copypaste_ipc::content_type::TEXT,
            false,
        );
        let stranger = crate::Keyring::from_secret(&[9u8; 32]);
        let data = export(&f.store, &stranger, 0, false).expect("unreadable rows are skipped");
        assert!(data.items.is_empty());
        assert_eq!(data.skipped_undecryptable, 1);
    }

    /// More rows than one page, so the paging loop is exercised rather than
    /// assumed.
    #[test]
    fn an_export_reads_past_the_first_page() {
        let f = fixture();
        let count = EXPORT_CHUNK + 7;
        for n in 0..count {
            f.add(&format!("item-{n}"));
        }
        assert_eq!(
            export_of(&f, false).items.len(),
            count as usize,
            "the paging loop stopped early"
        );
    }
}
