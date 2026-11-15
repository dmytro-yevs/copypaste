use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use copypaste_ipc::limits::ExportFrameBudget;
use copypaste_ipc::{
    ExportChunk, ExportData, ExportItem, TransferProgress, MAX_CONTENT_BYTES, MAX_FRAME_BYTES,
};
use tracing::{info, warn};

use crate::storage::{Store, StoreError, StoredItem};
use crate::{ItemKey, Keyring};

/// Serialized item bytes after which a streamed export hands over a chunk.
pub const EXPORT_CHUNK_BYTES: usize = 1024 * 1024;

/// Shortest pause between two progress reports of one streamed operation.
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

// A chunk closes once it reaches its target, so it holds at most one item
// beyond it, and JSON escaping grows a body at most sixfold.
const _: () = assert!(EXPORT_CHUNK_BYTES + 6 * MAX_CONTENT_BYTES + 64 * 1024 < MAX_FRAME_BYTES);

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("the history database could not be accessed")]
    Store(#[from] StoreError),
    #[error("included content or the aggregate export is too large")]
    ContentTooLarge,
    #[error("the export was abandoned before it finished")]
    Abandoned,
}

/// Read text history out as plaintext, counting everything it declines to include.
pub fn export(store: &Store, keyring: &Keyring, limit: u32) -> Result<ExportData, ExportError> {
    let key = keyring.item_key();
    let mut budget = ExportFrameBudget::new();
    let mut data = ExportData {
        items: Vec::new(),
        skipped_non_text: 0,
        skipped_undecryptable: 0,
    };

    store.visit_live_items(|row| -> Result<_, ExportError> {
        let Some(item) = export_item(row, &key, &mut data)? else {
            return Ok(ControlFlow::Continue(()));
        };
        budget
            .try_push(&item)
            .map_err(|_| ExportError::ContentTooLarge)?;
        data.items.push(item);
        Ok(if limit > 0 && data.items.len() as u32 >= limit {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        })
    })?;

    log_export(data.items.len(), &data);
    Ok(data)
}

/// [`export`] of the whole history in chunks of about [`EXPORT_CHUNK_BYTES`].
///
/// Each chunk reports the rows read so far, and one is handed over at least
/// every [`PROGRESS_INTERVAL`] while the read runs; the last one reports every
/// row. Returns the skip counts with no items. `sink` returning `false`
/// abandons the export.
pub fn export_chunks(
    store: &Store,
    keyring: &Keyring,
    mut sink: impl FnMut(ExportChunk) -> bool,
) -> Result<ExportData, ExportError> {
    let key = keyring.item_key();
    let mut summary = ExportData {
        items: Vec::new(),
        skipped_non_text: 0,
        skipped_undecryptable: 0,
    };
    let mut progress = TransferProgress {
        processed: 0,
        total: 0,
    };
    let mut chunk = Vec::new();
    let mut chunk_bytes = 0;
    let mut exported = 0;
    let mut reported = Instant::now();
    let mut hand_over = |items: &mut Vec<ExportItem>, progress| {
        if sink(ExportChunk {
            items: std::mem::take(items),
            progress,
        }) {
            Ok(())
        } else {
            Err(ExportError::Abandoned)
        }
    };

    let total = std::cell::Cell::new(0);
    store.visit_live_snapshot(
        |rows| total.set(rows),
        |row| -> Result<_, ExportError> {
            progress.processed += 1;
            progress.total = total.get().max(progress.processed);
            if let Some(item) = export_item(row, &key, &mut summary)? {
                chunk_bytes += serialized_len(&item);
                chunk.push(item);
                exported += 1;
            }
            if chunk_bytes >= EXPORT_CHUNK_BYTES || reported.elapsed() >= PROGRESS_INTERVAL {
                hand_over(&mut chunk, progress)?;
                chunk_bytes = 0;
                reported = Instant::now();
            }
            Ok(ControlFlow::Continue(()))
        },
    )?;
    progress.total = progress.processed;
    hand_over(&mut chunk, progress)?;

    log_export(exported, &summary);
    Ok(summary)
}

/// The export form of one live row, or `None` when the row is counted as skipped.
fn export_item(
    row: StoredItem,
    key: &ItemKey,
    counts: &mut ExportData,
) -> Result<Option<ExportItem>, ExportError> {
    if !copypaste_ipc::content_type::is_text(&row.content_type) {
        counts.skipped_non_text = counts.skipped_non_text.saturating_add(1);
        return Ok(None);
    }
    let plaintext = match crate::decrypt(&row.content_ciphertext, &row.nonce, key, &row.id) {
        Ok(plaintext) => plaintext,
        Err(e) => {
            warn!(id = %row.id, error = ?e, "export skipped an item that failed to decrypt");
            counts.skipped_undecryptable = counts.skipped_undecryptable.saturating_add(1);
            return Ok(None);
        }
    };
    if plaintext.len() > MAX_CONTENT_BYTES {
        return Err(ExportError::ContentTooLarge);
    }
    Ok(Some(ExportItem {
        privacy: row.clipboard_privacy(),
        content: std::mem::take(&mut *crate::clipboard_payload::text_from_bytes(plaintext)),
        content_type: row.content_type,
        created_at: row.created_at,
        pinned: row.pinned,
    }))
}

fn serialized_len(item: &ExportItem) -> usize {
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count(0);
    // Writing to a counter cannot fail, and the item holds no map keys.
    let _ = serde_json::to_writer(&mut count, item);
    count.0
}

fn log_export(exported: usize, counts: &ExportData) {
    // The count only — never content, and never a sample.
    info!(
        items = exported,
        skipped_non_text = counts.skipped_non_text,
        skipped_undecryptable = counts.skipped_undecryptable,
        "exported history"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::testkit::{fixture, Fixture};

    fn seed_legacy_text(f: &Fixture, content: &[u8], content_type: &str) {
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
                search_text: Some(String::from_utf8_lossy(content).into_owned()),
                created_at: crate::now_ms(),
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .unwrap();
    }

    fn export_of(f: &Fixture) -> ExportData {
        export(&f.store, &f.keyring, 0).expect("the store must read")
    }

    #[test]
    fn an_export_carries_the_plaintext_its_stamp_and_its_pin() {
        let f = fixture();
        let id = f.add("first");
        f.store.set_pinned(&id, true).unwrap();
        let created_at = f.store.get(&id).unwrap().unwrap().created_at;
        f.add("second");

        let data = export_of(&f);
        assert_eq!(data.items.len(), 2);
        let first = data
            .items
            .iter()
            .find(|i| i.content == "first")
            .expect("the pinned item");
        assert_eq!(first.created_at, created_at);
        assert!(first.pinned);
    }

    #[test]
    fn credential_shaped_text_is_exported() {
        let f = fixture();
        f.add("ordinary");
        f.add("AKIAIOSFODNN7EXAMPLE");

        let data = export_of(&f);
        assert_eq!(data.items.len(), 2);
        let rendered = serde_json::to_string(&data).unwrap();
        assert!(
            rendered.contains("AKIAIOSFODNN7EXAMPLE"),
            "credential-shaped text did not reach the export"
        );
    }

    #[test]
    fn the_skip_counts_are_always_present() {
        let f = fixture();
        f.add("ordinary");
        let json = serde_json::to_value(export_of(&f)).unwrap();
        for field in ["skipped_non_text", "skipped_undecryptable"] {
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

        let data = export_of(&f);
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

        let data = export(&f.store, &stranger, 0).expect("the store still reads");
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
        assert_eq!(export(&f.store, &f.keyring, 2).unwrap().items.len(), 2);
        assert_eq!(export(&f.store, &f.keyring, 0).unwrap().items.len(), 5);
    }

    #[test]
    fn an_explicit_limit_stops_before_a_following_oversized_row() {
        let f = fixture();
        seed_legacy_text(
            &f,
            "\u{1}"
                .repeat(copypaste_ipc::MAX_CONTENT_BYTES + 1)
                .as_bytes(),
            copypaste_ipc::content_type::TEXT,
        );
        let first = f.add("first");
        f.store.set_pinned(&first, true).unwrap();

        let data = export(&f.store, &f.keyring, 1).unwrap();
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.items[0].content, "first");
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
            export(&f.store, &f.keyring, 0),
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
    fn an_unreadable_oversized_legacy_row_is_still_counted_not_refused_for_size() {
        let f = fixture();
        seed_legacy_text(
            &f,
            "\u{1}"
                .repeat(copypaste_ipc::MAX_CONTENT_BYTES + 1)
                .as_bytes(),
            copypaste_ipc::content_type::TEXT,
        );
        let stranger = crate::Keyring::from_secret(&[9u8; 32]);
        let data = export(&f.store, &stranger, 0).expect("unreadable rows are skipped");
        assert!(data.items.is_empty());
        assert_eq!(data.skipped_undecryptable, 1);
    }

    #[test]
    fn legal_items_that_overflow_the_aggregate_refuse_without_changing_ciphertext() {
        let f = fixture();
        let content = "\u{1}".repeat(copypaste_ipc::MAX_CONTENT_BYTES);
        seed_legacy_text(&f, content.as_bytes(), copypaste_ipc::content_type::TEXT);
        let first_id = f.store.list(10, 0).unwrap()[0].id.clone();
        f.store.set_pinned(&first_id, true).unwrap();
        let second_id = format!("aggregate-second-{}", crate::now_ms());
        let second_content = format!(
            "\u{2}{}",
            "\u{1}".repeat(copypaste_ipc::MAX_CONTENT_BYTES - 1)
        );
        let key = f.keyring.item_key();
        let (nonce, content_ciphertext) =
            crate::encrypt(second_content.as_bytes(), &key, &second_id).unwrap();
        f.store
            .insert(crate::NewItem {
                id: second_id.clone(),
                content_ciphertext,
                nonce,
                content_type: copypaste_ipc::content_type::TEXT.to_string(),
                content_hash: crate::compute_content_hash(second_content.as_bytes()),
                search_text: Some(second_content),
                created_at: crate::now_ms(),
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .unwrap();
        let before: Vec<_> = f
            .store
            .list(10, 0)
            .unwrap()
            .into_iter()
            .map(|row| (row.id, row.content_ciphertext))
            .collect();

        assert!(matches!(
            export(&f.store, &f.keyring, 0),
            Err(ExportError::ContentTooLarge)
        ));
        let after: Vec<_> = f
            .store
            .list(10, 0)
            .unwrap()
            .into_iter()
            .map(|row| (row.id, row.content_ciphertext))
            .collect();
        assert_eq!(after, before);
    }

    fn streamed(f: &Fixture) -> (Vec<ExportChunk>, ExportData) {
        let mut chunks = Vec::new();
        let summary = export_chunks(&f.store, &f.keyring, |chunk| {
            chunks.push(chunk);
            true
        })
        .expect("the store must read");
        (chunks, summary)
    }

    #[test]
    fn a_streamed_export_carries_the_same_items_and_counts_as_one_reply() {
        let f = fixture();
        let pinned = f.add("pinned");
        f.store.set_pinned(&pinned, true).unwrap();
        f.add("second");
        f.add_typed("\u{fffd}PNG", "image/png");
        f.add("third \u{1} with a control character");

        let (chunks, summary) = streamed(&f);
        let items: Vec<_> = chunks.into_iter().flat_map(|chunk| chunk.items).collect();
        let whole = export_of(&f);

        assert!(summary.items.is_empty());
        assert_eq!(
            serde_json::to_value(&items).unwrap(),
            serde_json::to_value(&whole.items).unwrap()
        );
        assert_eq!(summary.skipped_non_text, whole.skipped_non_text);
        assert_eq!(summary.skipped_undecryptable, whole.skipped_undecryptable);
    }

    #[test]
    fn streamed_progress_is_monotonic_and_ends_with_every_row() {
        let f = fixture();
        for n in 0..5 {
            f.add(&format!("item-{n}"));
        }
        f.add_typed("\u{fffd}PNG", "image/png");

        let (chunks, _) = streamed(&f);
        let reported: Vec<_> = chunks.iter().map(|chunk| chunk.progress).collect();

        assert!(reported
            .windows(2)
            .all(|pair| pair[0].processed <= pair[1].processed));
        assert!(reported.iter().all(|p| p.processed <= p.total));
        assert_eq!(
            reported.last(),
            Some(&TransferProgress {
                processed: 6,
                total: 6
            })
        );
    }

    #[test]
    fn an_empty_history_streams_one_complete_chunk() {
        let (chunks, summary) = streamed(&fixture());
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].items.is_empty());
        assert_eq!(chunks[0].progress.processed, chunks[0].progress.total);
        assert_eq!(summary.skipped_non_text, 0);
    }

    /// Larger than one IPC frame in total, which the single-reply export
    /// refuses; streamed, no chunk holds more than one such item.
    #[test]
    fn an_export_larger_than_one_frame_streams_in_bounded_chunks() {
        let f = fixture();
        let body = "\u{1}".repeat(copypaste_ipc::MAX_CONTENT_BYTES);
        seed_legacy_text(&f, body.as_bytes(), copypaste_ipc::content_type::TEXT);
        f.add(&"\u{1}".repeat(copypaste_ipc::MAX_CONTENT_BYTES - 1));
        assert!(matches!(
            export(&f.store, &f.keyring, 0),
            Err(ExportError::ContentTooLarge)
        ));

        let (chunks, _) = streamed(&f);
        let largest = chunks
            .iter()
            .map(|chunk| serde_json::to_vec(&chunk.items).unwrap().len())
            .max()
            .unwrap();

        assert_eq!(chunks.iter().map(|c| c.items.len()).sum::<usize>(), 2);
        assert!(chunks.iter().all(|chunk| chunk.items.len() <= 1));
        assert!(largest < copypaste_ipc::MAX_FRAME_BYTES);
    }

    #[test]
    fn small_items_are_grouped_up_to_the_chunk_target() {
        let f = fixture();
        let body = "x".repeat(300 * 1024);
        for n in 0..8 {
            f.add(&format!("{n}{body}"));
        }

        let (chunks, _) = streamed(&f);
        let item_chunks: Vec<_> = chunks.iter().filter(|c| !c.items.is_empty()).collect();

        assert!(item_chunks.len() >= 2, "{} chunks", item_chunks.len());
        assert!(item_chunks.iter().all(|chunk| {
            serde_json::to_vec(&chunk.items).unwrap().len() < EXPORT_CHUNK_BYTES + body.len() + 1024
        }));
    }

    #[test]
    fn a_reader_that_stops_abandons_the_export() {
        let f = fixture();
        f.add("only");
        let result = export_chunks(&f.store, &f.keyring, |_| false);
        assert!(matches!(result, Err(ExportError::Abandoned)));
    }
}
