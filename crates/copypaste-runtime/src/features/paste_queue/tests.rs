use std::path::Path;
use std::sync::{Arc, Mutex};

use copypaste_core::{ClipboardPayload, ClipboardWriteError, Keyring, NewItem};
use copypaste_ipc::{content_type, ConfigPatch, Method};

use super::*;
use crate::host::{ClipboardWriter, InProcessHost, WriteOptions};

#[derive(Default)]
struct Writes(Mutex<Vec<(String, String, WriteOptions)>>);

impl ClipboardWriter for Writes {
    fn write(
        &self,
        payload: &ClipboardPayload,
        content_type: &str,
        options: WriteOptions,
    ) -> Result<(), ClipboardWriteError> {
        let text = match payload {
            ClipboardPayload::Text(text) => text.as_str().to_owned(),
            _ => String::from("<binary>"),
        };
        self.0
            .lock()
            .unwrap()
            .push((text, content_type.to_owned(), options));
        Ok(())
    }
}

impl Writes {
    fn texts(&self) -> Vec<String> {
        self.0.lock().unwrap().iter().map(|w| w.0.clone()).collect()
    }
}

fn open(dir: &Path, writes: &Arc<Writes>) -> Arc<Runtime> {
    Arc::new(
        Runtime::open_with_keyring(
            dir,
            "queue",
            0,
            Arc::new(InProcessHost(writes.clone())),
            Arc::new(Keyring::from_secret(&[7; 32])),
        )
        .unwrap(),
    )
}

fn runtime() -> (Arc<Runtime>, Arc<Writes>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let writes = Arc::new(Writes::default());
    (open(dir.path(), &writes), writes, dir)
}

fn configure(runtime: &Runtime, change: impl FnOnce(&mut PasteQueueSettings)) {
    let mut settings = runtime.settings.get().paste_queue.clone();
    change(&mut settings);
    runtime
        .settings
        .apply(&ConfigPatch {
            paste_queue: Some(settings),
            ..Default::default()
        })
        .unwrap();
}

fn request(runtime: &Runtime, op: PasteQueueOp) -> Response {
    runtime.request_blocking(1, Method::PasteQueue { op })
}

fn state(runtime: &Runtime, op: PasteQueueOp) -> PasteQueueState {
    match request(runtime, op).data {
        Some(ResponseData::PasteQueue(state)) => state,
        other => panic!("unexpected {other:?}"),
    }
}

fn deliver(runtime: &Runtime, target: QueueTarget, mode: QueueDeliveryMode) -> QueueDelivery {
    let op = PasteQueueOp::Deliver {
        target,
        plain_text: None,
        mode,
    };
    match request(runtime, op).data {
        Some(ResponseData::QueueDelivery(delivery)) => delivery,
        other => panic!("unexpected {other:?}"),
    }
}

fn next(runtime: &Runtime) -> QueueDelivery {
    deliver(runtime, QueueTarget::Next, QueueDeliveryMode::Clipboard)
}

fn collecting(runtime: &Runtime, on: bool) -> PasteQueueState {
    state(
        runtime,
        PasteQueueOp::SetCollecting {
            on,
            on_finished: QueueRestart::StartFresh,
        },
    )
}

fn capture(runtime: &Runtime, text: &str) -> String {
    let item = copypaste_core::ingest::ingest(
        &runtime.store,
        &runtime.keyring,
        text,
        content_type::TEXT,
        &runtime.settings.config(),
    )
    .unwrap()
    .into_item();
    runtime.note_capture(item.created_at, &item.id);
    item.id
}

fn add_clips(runtime: &Runtime, ids: &[&str]) -> PasteQueueState {
    let sources = ids
        .iter()
        .map(|id| QueueSource::Clip { id: (*id).into() })
        .collect();
    state(runtime, PasteQueueOp::Add { sources })
}

fn previews(state: &PasteQueueState) -> Vec<&str> {
    state.entries.iter().map(|e| e.preview.as_str()).collect()
}

fn statuses(state: &PasteQueueState) -> Vec<QueueEntryStatus> {
    state.entries.iter().map(|e| e.status).collect()
}

#[test]
fn collected_copies_paste_in_order_once_and_then_nothing() {
    let (runtime, writes, _dir) = runtime();
    collecting(&runtime, true);
    for text in ["one", "two", "three", "four", "five"] {
        capture(&runtime, text);
    }
    let collected = state(&runtime, PasteQueueOp::Get);
    assert_eq!(
        previews(&collected),
        ["one", "two", "three", "four", "five"]
    );
    let rows = runtime.store.count().unwrap();

    for _ in 0..5 {
        assert!(next(&runtime).nothing_to_paste.is_none());
    }
    assert_eq!(writes.texts(), ["one", "two", "three", "four", "five"]);
    assert!(writes.0.lock().unwrap().iter().all(|w| w.2.transient));
    assert_eq!(
        runtime.store.count().unwrap(),
        rows,
        "deliveries add no rows"
    );

    let after = next(&runtime);
    assert_eq!(after.nothing_to_paste, Some(NothingToPaste::Finished));
    assert_eq!(writes.texts().len(), 5, "an empty press writes nothing");
    let finished = state(&runtime, PasteQueueOp::Get);
    assert_eq!(finished.phase, QueuePhase::Finished);
    assert!(!finished.collecting, "finishing stops collecting");
    assert!(statuses(&finished)
        .iter()
        .all(|s| *s == QueueEntryStatus::Pasted));
}

#[test]
fn rapid_presses_each_take_the_next_entry_exactly_once() {
    let (runtime, writes, _dir) = runtime();
    let ids: Vec<String> = (1..=10)
        .map(|n| capture(&runtime, &n.to_string()))
        .collect();
    add_clips(
        &runtime,
        &ids.iter().map(String::as_str).collect::<Vec<_>>(),
    );

    let presses: Vec<_> = (0..10)
        .map(|_| {
            let runtime = Arc::clone(&runtime);
            std::thread::spawn(move || next(&runtime).entry_ids)
        })
        .collect();
    let mut delivered: Vec<String> = presses
        .into_iter()
        .flat_map(|press| press.join().unwrap())
        .collect();
    delivered.sort();
    delivered.dedup();
    assert_eq!(delivered.len(), 10);
    let expected: Vec<String> = (1..=10).map(|n| n.to_string()).collect();
    assert_eq!(writes.texts(), expected);
}

#[test]
fn collecting_off_keeps_entries_and_a_clear_can_be_undone() {
    let (runtime, _writes, _dir) = runtime();
    collecting(&runtime, true);
    capture(&runtime, "a");
    capture(&runtime, "b");
    next(&runtime);
    collecting(&runtime, false);
    capture(&runtime, "not collected");
    let before = collecting(&runtime, true);
    assert_eq!(previews(&before), ["a", "b"]);

    let cleared = state(&runtime, PasteQueueOp::Clear);
    assert!(cleared.entries.is_empty());
    assert!(cleared.undo_until.is_some());
    let restored = state(&runtime, PasteQueueOp::UndoClear);
    assert_eq!(previews(&restored), ["a", "b"]);
    assert_eq!(statuses(&restored), statuses(&before));

    state(&runtime, PasteQueueOp::Clear);
    runtime.deadlines.advance(UNDO_MS + 1);
    runtime.sweep_deadlines();
    assert!(state(&runtime, PasteQueueOp::Get).undo_until.is_none());
    assert!(!request(&runtime, PasteQueueOp::UndoClear).ok);
}

#[test]
fn start_insertion_and_reverse_set_the_paste_order() {
    let (runtime, writes, _dir) = runtime();
    configure(&runtime, |s| s.insert_at_start = true);
    collecting(&runtime, true);
    for text in ["A", "B", "C"] {
        capture(&runtime, text);
    }
    assert_eq!(
        previews(&state(&runtime, PasteQueueOp::Get)),
        ["C", "B", "A"]
    );
    next(&runtime);
    assert_eq!(writes.texts(), ["C"]);

    let reversed = state(&runtime, PasteQueueOp::Reverse);
    assert_eq!(previews(&reversed), ["C", "A", "B"], "pasted entries stay");
    next(&runtime);
    assert_eq!(writes.texts(), ["C", "A"]);
}

#[test]
fn explicit_duplicates_stay_and_a_double_capture_collapses() {
    let (runtime, writes, _dir) = runtime();
    let id = capture(&runtime, "same");
    assert_eq!(add_clips(&runtime, &[&id, &id]).entries.len(), 2);
    next(&runtime);
    next(&runtime);
    assert_eq!(writes.texts(), ["same", "same"]);

    state(&runtime, PasteQueueOp::Clear);
    collecting(&runtime, true);
    capture(&runtime, "twice");
    capture(&runtime, "twice");
    assert_eq!(state(&runtime, PasteQueueOp::Get).entries.len(), 1);
    runtime.deadlines.advance(COLLAPSE_MS + 1);
    capture(&runtime, "twice");
    assert_eq!(state(&runtime, PasteQueueOp::Get).entries.len(), 2);
}

#[test]
fn copies_capture_policy_refuses_never_join_the_queue() {
    let (runtime, _writes, _dir) = runtime();
    collecting(&runtime, true);
    runtime
        .settings
        .apply(&ConfigPatch {
            private_mode: Some(true),
            ..Default::default()
        })
        .unwrap();
    assert!(runtime.capture_text("paused copy").is_err());
    let refused = request(
        &runtime,
        PasteQueueOp::Add {
            sources: vec![QueueSource::Text {
                text: "shared".into(),
                via: QueueTextVia::Share,
            }],
        },
    );
    assert_eq!(refused.error_code, Some(ErrorCode::InvalidRequest));
    assert!(state(&runtime, PasteQueueOp::Get).entries.is_empty());
    assert_eq!(runtime.store.count().unwrap(), 0);
}

#[test]
fn shared_text_is_stored_in_history_and_queued_once() {
    let (runtime, _writes, _dir) = runtime();
    collecting(&runtime, true);
    let added = state(
        &runtime,
        PasteQueueOp::Add {
            sources: vec![QueueSource::Text {
                text: "selected words".into(),
                via: QueueTextVia::SelectionAction,
            }],
        },
    );
    assert_eq!(previews(&added), ["selected words"]);
    assert_eq!(added.entries[0].added_via, QueueAddedVia::SelectionAction);
    assert_eq!(runtime.store.count().unwrap(), 1);
}

#[test]
fn step_back_and_end_behaviors() {
    let (runtime, writes, _dir) = runtime();
    let a = capture(&runtime, "A");
    let b = capture(&runtime, "B");
    add_clips(&runtime, &[&a, &b]);
    next(&runtime);
    let back = state(&runtime, PasteQueueOp::StepBack);
    assert_eq!(statuses(&back)[0], QueueEntryStatus::Pending);
    next(&runtime);
    assert_eq!(writes.texts(), ["A", "A"]);

    next(&runtime);
    configure(&runtime, |s| s.end_behavior = QueueEndBehavior::Loop);
    next(&runtime);
    configure(&runtime, |s| s.end_behavior = QueueEndBehavior::KeepLast);
    next(&runtime);
    next(&runtime);
    next(&runtime);
    assert_eq!(writes.texts(), ["A", "A", "B", "A", "B", "B", "B"]);
}

#[test]
fn starting_fresh_after_finishing_drops_only_pasted_entries() {
    let (runtime, _writes, _dir) = runtime();
    let a = capture(&runtime, "A");
    let b = capture(&runtime, "B");
    add_clips(&runtime, &[&a, &b]);
    next(&runtime);
    state(&runtime, PasteQueueOp::Skip);
    assert_eq!(
        state(&runtime, PasteQueueOp::Get).phase,
        QueuePhase::Finished
    );
    let appended = state(
        &runtime,
        PasteQueueOp::SetCollecting {
            on: true,
            on_finished: QueueRestart::Append,
        },
    );
    assert_eq!(appended.entries.len(), 2);
    collecting(&runtime, false);
    assert!(collecting(&runtime, true).entries.is_empty());
}

#[test]
fn paste_all_joins_text_and_refuses_a_mixed_queue() {
    let (runtime, writes, _dir) = runtime();
    configure(&runtime, |s| s.separator = QueueSeparator::Tab);
    let ids: Vec<String> = ["A", "B", "C"]
        .iter()
        .map(|t| capture(&runtime, t))
        .collect();
    add_clips(
        &runtime,
        &ids.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let all = deliver(&runtime, QueueTarget::All, QueueDeliveryMode::Clipboard);
    assert_eq!(all.entry_ids.len(), 3);
    assert_eq!(writes.texts(), ["A\tB\tC"]);
    let pasted = state(&runtime, PasteQueueOp::Get);
    assert!(statuses(&pasted)
        .iter()
        .all(|s| *s == QueueEntryStatus::Pasted));

    let image = runtime
        .store
        .insert(NewItem {
            id: "image".into(),
            content_ciphertext: vec![0; 32],
            nonce: vec![1; 24],
            content_type: content_type::IMAGE_PNG.into(),
            content_hash: "image-hash".into(),
            search_text: None,
            created_at: 1,
            app_bundle_id: None,
            app_name: None,
            payload_metadata: None,
        })
        .unwrap();
    let text = capture(&runtime, "text");
    add_clips(&runtime, &[&image.id, &text]);
    let mixed = deliver(&runtime, QueueTarget::All, QueueDeliveryMode::Clipboard);
    assert_eq!(mixed.nothing_to_paste, Some(NothingToPaste::Mixed));
    assert_eq!(writes.texts().len(), 1);

    let mut settings = PasteQueueSettings::default();
    assert_eq!(separator(&settings, true), "\r\n");
    assert_eq!(separator(&settings, false), "\n");
    settings.separator = QueueSeparator::Custom;
    settings.custom_separator = r" \t|\n".into();
    assert_eq!(separator(&settings, false), " \t|\n");
}

#[test]
fn an_edit_changes_what_is_pasted_but_not_the_history_item() {
    let (runtime, writes, _dir) = runtime();
    let id = capture(&runtime, "original");
    let entry = add_clips(&runtime, &[&id]).entries[0].entry_id.clone();
    let edited = state(
        &runtime,
        PasteQueueOp::Edit {
            entry_id: entry,
            text: Some("edited".into()),
        },
    );
    assert!(edited.entries[0].edited);
    assert_eq!(previews(&edited), ["edited"]);
    next(&runtime);
    assert_eq!(writes.texts(), ["edited"]);
    let stored = runtime.store.get(&id).unwrap().unwrap();
    let (item, _) = runtime.to_wire(stored).unwrap();
    assert_eq!(item.content, "original");
}

#[test]
fn a_deleted_clip_is_skipped_with_a_notice() {
    let (runtime, writes, _dir) = runtime();
    let gone = capture(&runtime, "gone");
    let kept = capture(&runtime, "kept");
    add_clips(&runtime, &[&gone, &kept]);
    runtime.store.delete(&gone).unwrap();
    assert_eq!(
        statuses(&state(&runtime, PasteQueueOp::Get))[0],
        QueueEntryStatus::Unavailable
    );
    let delivery = next(&runtime);
    assert_eq!(delivery.notices.unavailable, 1);
    assert_eq!(writes.texts(), ["kept"]);
}

#[test]
fn clearing_history_clears_the_queue() {
    let (runtime, _writes, _dir) = runtime();
    let id = capture(&runtime, "entry");
    add_clips(&runtime, &[&id]);
    assert!(
        runtime
            .request_blocking(1, Method::DeleteAll { through: None })
            .ok
    );
    assert!(state(&runtime, PasteQueueOp::Get).entries.is_empty());
}

#[test]
fn a_reopened_runtime_keeps_its_entries_with_collecting_off() {
    let dir = tempfile::tempdir().unwrap();
    let writes = Arc::new(Writes::default());
    let before = {
        let runtime = open(dir.path(), &writes);
        collecting(&runtime, true);
        capture(&runtime, "first");
        capture(&runtime, "second");
        next(&runtime);
        let entry = state(&runtime, PasteQueueOp::Get).entries[1]
            .entry_id
            .clone();
        state(
            &runtime,
            PasteQueueOp::Edit {
                entry_id: entry,
                text: Some("changed".into()),
            },
        )
    };
    let runtime = open(dir.path(), &writes);
    let after = state(&runtime, PasteQueueOp::Get);
    assert!(!after.collecting);
    assert_eq!(previews(&after), previews(&before));
    assert_eq!(statuses(&after), statuses(&before));
    assert!(after.entries[1].edited);
}

#[test]
fn collecting_stops_after_the_idle_period_and_keeps_entries() {
    let (runtime, _writes, _dir) = runtime();
    configure(&runtime, |s| s.idle_stop_minutes = 5);
    collecting(&runtime, true);
    capture(&runtime, "kept");
    runtime.deadlines.advance(4 * 60_000);
    runtime.sweep_deadlines();
    assert!(state(&runtime, PasteQueueOp::Get).collecting);
    runtime.deadlines.advance(60_000 + 1);
    runtime.sweep_deadlines();
    let stopped = state(&runtime, PasteQueueOp::Get);
    assert!(!stopped.collecting);
    assert_eq!(previews(&stopped), ["kept"]);
}

#[test]
fn a_full_queue_stops_appending_without_dropping_entries() {
    let (runtime, _writes, _dir) = runtime();
    configure(&runtime, |s| s.max_entries = 10);
    collecting(&runtime, true);
    for n in 0..11 {
        capture(&runtime, &format!("item {n}"));
    }
    let full = state(&runtime, PasteQueueOp::Get);
    assert!(full.full);
    assert_eq!(full.entries.len(), 10);
    assert_eq!(full.entries[0].preview, "item 0");
}

#[test]
fn an_editor_commit_marks_the_entry_only_when_confirmed() {
    let (runtime, writes, _dir) = runtime();
    let a = capture(&runtime, "A");
    let b = capture(&runtime, "B");
    add_clips(&runtime, &[&a, &b]);
    let first = deliver(&runtime, QueueTarget::Next, QueueDeliveryMode::Return);
    assert_eq!(first.text.as_deref(), Some("A"));
    let second = deliver(&runtime, QueueTarget::Next, QueueDeliveryMode::Return);
    assert_eq!(
        second.text.as_deref(),
        Some("B"),
        "a reserved entry is skipped"
    );
    assert!(writes.texts().is_empty(), "an editor commit never writes");

    let rejected = state(
        &runtime,
        PasteQueueOp::Confirm {
            token: first.token.unwrap(),
            outcome: QueueDeliveryOutcome::Rejected,
        },
    );
    assert_eq!(statuses(&rejected)[0], QueueEntryStatus::Pending);
    let confirmed = state(
        &runtime,
        PasteQueueOp::Confirm {
            token: second.token.unwrap(),
            outcome: QueueDeliveryOutcome::Inserted,
        },
    );
    assert_eq!(
        statuses(&confirmed),
        [QueueEntryStatus::Pending, QueueEntryStatus::Pasted]
    );
}
