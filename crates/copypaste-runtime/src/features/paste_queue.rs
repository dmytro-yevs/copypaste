//! The device-local paste queue: collecting, ordering and ordered delivery.
//!
//! The store holds the entries. This module holds what must not survive a
//! restart: whether collecting is on, the window that collapses a double
//! capture, the undo of a clear, and deliveries still waiting for their
//! confirmation. Every change moves the store revision and emits one
//! `PasteQueue` event. An idle queue costs nothing: the idle stop and the undo
//! window are one-shot deadlines, and payloads are opened only to deliver.
//!
//! A clipboard delivery marks its entries pasted at once; a rejected paste
//! puts them back. An editor commit (`Return`) reserves its entries and marks
//! them only when the commit is confirmed.

use std::cell::Cell;
use std::collections::hash_map::RandomState;
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::hash::BuildHasher;
use std::sync::{Mutex, MutexGuard};

use copypaste_core::storage::paste_queue::{
    content_kind_of, ClearedQueue, NewQueueEntry, QueueEntryContent, QueueEntryRow,
    QueueSourceKind, Zeroizing,
};
use copypaste_core::StoreError;
use copypaste_ipc::config::{PasteQueueSettings, QueueEndBehavior, QueueSeparator};
use copypaste_ipc::paste_queue::{
    NothingToPaste, PasteQueueOp, PasteQueueState, QueueAddedVia, QueueContentKind, QueueDelivery,
    QueueDeliveryMode, QueueDeliveryOutcome, QueueEntry, QueueEntryStatus, QueueNotices,
    QueuePhase, QueueRestart, QueueSource, QueueTarget, QueueTextVia,
};
use copypaste_ipc::{ErrorCode, Response, ResponseData, SemanticKind};

use super::delivery::DeliveryOptions;
use crate::deadlines::DeadlineKey;
use crate::messages::{storage_error, MSG_EMPTY_CONTENT, MSG_NOT_FOUND};
use crate::Runtime;

/// A capture identical to the previous collected one within this window is
/// the same copy written twice.
const COLLAPSE_MS: i64 = 1_500;
const UNDO_MS: i64 = 10_000;
const STALE_MS: i64 = 24 * 60 * 60 * 1000;
/// The pause before the clipboard is put back after the queue finishes.
const FINISH_RESTORE_MS: u32 = 500;
/// Unconfirmed deliveries remembered; the oldest is forgotten first.
const MAX_IN_FLIGHT: usize = 16;
/// The longest override or snapshot text.
const MAX_TEXT_BYTES: usize = 1024 * 1024;
const IDLE_OWNER: &str = "paste_queue_idle";
const UNDO_OWNER: &str = "paste_queue_undo";
const CRLF: bool = cfg!(windows);

const MSG_FULL: &str = "the paste queue is full";
const MSG_DISABLED: &str = "the paste queue is turned off";
const MSG_NOT_STORED: &str = "capture settings do not allow storing this content";
const MSG_NOT_TEXT: &str = "only text entries can be edited or inserted as text";
const MSG_NO_UNDO: &str = "there is no cleared queue to restore";
const MSG_UNKNOWN_DELIVERY: &str = "that delivery is not pending";
const MSG_SNIPPETS: &str = "snippets cannot be added to the paste queue yet";
const MSG_MANY_FILES: &str = "pasting several files at once is not available yet";

thread_local! {
    /// Set while an explicit add captures its text, so collecting does not
    /// append the same item a second time.
    static ADDING: Cell<bool> = const { Cell::new(false) };
}

/// The queue state held in memory, behind one lock that serializes every
/// queue operation.
#[derive(Default)]
pub(crate) struct PasteQueue(Mutex<Memory>);

#[derive(Default)]
struct Memory {
    collecting: bool,
    /// Content hash and time of the last collected capture.
    last_collected: Option<(String, i64)>,
    undo: Option<(ClearedQueue, i64)>,
    in_flight: VecDeque<InFlight>,
    tokens: u64,
    /// What the clipboard held before the first delivery of this run.
    run_start: Option<Option<String>>,
}

struct InFlight {
    token: String,
    entry_ids: Vec<String>,
    /// Marked pasted only when confirmed (an editor commit).
    awaiting: bool,
    /// The clipboard restore to schedule once this finishing paste lands.
    restore: Option<String>,
}

impl PasteQueue {
    fn lock(&self) -> MutexGuard<'_, Memory> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// How "Paste all" joins entries: the separator with the platform's line
/// ending. Custom separators accept `\n`, `\t` and `\\` escapes.
pub(crate) fn separator(settings: &PasteQueueSettings, crlf: bool) -> String {
    let base = match settings.separator {
        QueueSeparator::NewLine => "\n".to_owned(),
        QueueSeparator::BlankLine => "\n\n".to_owned(),
        QueueSeparator::Tab => "\t".to_owned(),
        QueueSeparator::Space => " ".to_owned(),
        QueueSeparator::Comma => ",".to_owned(),
        QueueSeparator::CommaSpace => ", ".to_owned(),
        QueueSeparator::None => String::new(),
        QueueSeparator::Custom => unescape(&settings.custom_separator),
    };
    if crlf {
        base.replace('\n', "\r\n")
    } else {
        base
    }
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => {
                out.push('\\');
                if other != '\\' {
                    out.push(other);
                }
            }
            None => out.push('\\'),
        }
    }
    out
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace("\r\n", "<br>")
        .replace('\n', "<br>")
}

fn is_textual(kind: QueueContentKind) -> bool {
    matches!(
        kind,
        QueueContentKind::Text | QueueContentKind::RichText | QueueContentKind::Link
    )
}

fn delivery_of(nothing: NothingToPaste, notices: QueueNotices) -> QueueDelivery {
    QueueDelivery {
        token: None,
        entry_ids: Vec::new(),
        text: None,
        notices,
        finished: false,
        nothing_to_paste: Some(nothing),
    }
}

/// A queue operation's failure, already in wire form.
type Failure = Box<Response>;

impl Runtime {
    pub(crate) fn paste_queue(&self, id: u64, op: PasteQueueOp) -> Response {
        let mut memory = self.paste_queue.lock();
        let changed = match op {
            PasteQueueOp::Get => Ok(None),
            PasteQueueOp::Deliver {
                target,
                plain_text,
                mode,
            } => {
                return match self.deliver_queue(id, &mut memory, &target, plain_text, mode) {
                    Ok((delivery, revision)) => {
                        if let Some(revision) = revision {
                            self.note_paste_queue_changed(revision);
                        }
                        Response::ok(id, ResponseData::QueueDelivery(delivery))
                    }
                    Err(failure) => *failure,
                };
            }
            PasteQueueOp::SetCollecting { on, on_finished } => {
                self.set_collecting(id, &mut memory, on, on_finished)
            }
            PasteQueueOp::Add { sources } => self.add_sources(id, sources),
            PasteQueueOp::Remove { entry_ids } => self
                .store
                .paste_queue_remove(&entry_ids)
                .map(Some)
                .map_err(|error| failed(id, &error)),
            PasteQueueOp::RemovePasted => self
                .store
                .paste_queue_remove_finished()
                .map(Some)
                .map_err(|error| failed(id, &error)),
            PasteQueueOp::Clear => self.clear_queue(id, &mut memory),
            PasteQueueOp::UndoClear => self.undo_clear(id, &mut memory),
            PasteQueueOp::Reverse => self
                .store
                .paste_queue_reverse()
                .map(Some)
                .map_err(|error| failed(id, &error)),
            PasteQueueOp::Move { entry_id, to_index } => {
                match self.store.paste_queue_move(&entry_id, to_index as usize) {
                    Ok(Some(revision)) => Ok(Some(revision)),
                    Ok(None) => Err(refuse(id, ErrorCode::NotFound, MSG_NOT_FOUND)),
                    Err(error) => Err(failed(id, &error)),
                }
            }
            PasteQueueOp::Edit { entry_id, text } => self.edit_entry(id, &entry_id, text),
            PasteQueueOp::Confirm { token, outcome } => {
                self.confirm(id, &mut memory, &token, outcome)
            }
            PasteQueueOp::StepBack => self
                .store
                .paste_queue_step_back()
                .map_err(|error| failed(id, &error)),
            PasteQueueOp::Skip => self.skip(id, &memory),
        };
        match changed {
            Ok(revision) => {
                if let Some(revision) = revision {
                    self.note_paste_queue_changed(revision);
                }
                match self.queue_state(&memory) {
                    Ok(state) => Response::ok(id, ResponseData::PasteQueue(state)),
                    Err(error) => *failed(id, &error),
                }
            }
            Err(failure) => *failure,
        }
    }

    /// A local capture was committed; collecting appends it.
    pub(crate) fn paste_queue_captured(&self, item_id: &str) {
        if ADDING.with(Cell::get) {
            return;
        }
        let mut memory = self.paste_queue.lock();
        if !memory.collecting {
            return;
        }
        let settings = self.settings.get().paste_queue.clone();
        if !settings.enabled {
            return;
        }
        let (hash, content_type) = match self.store.paste_queue_clip(item_id) {
            Ok(Some(facts)) => facts,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(?error, "a capture could not be read for the paste queue");
                return;
            }
        };
        let now = self.deadlines.now();
        if memory
            .last_collected
            .as_ref()
            .is_some_and(|(last, at)| *last == hash && now.saturating_sub(*at) < COLLAPSE_MS)
        {
            return;
        }
        let entry = NewQueueEntry {
            source: QueueSourceKind::Clip,
            clip_id: Some(item_id.to_owned()),
            snapshot: None,
            content_kind: content_kind_of(&content_type),
            added_via: QueueAddedVia::Collect,
            rendered_at: None,
        };
        match self.store.paste_queue_add(
            &self.keyring.item_key(),
            &[entry],
            settings.insert_at_start,
            settings.max_entries,
            now,
            true,
        ) {
            Ok(Some((_, revision))) => {
                memory.last_collected = Some((hash, now));
                self.arm_idle_stop(settings.idle_stop_minutes, now);
                self.note_paste_queue_changed(revision);
            }
            Ok(None) => tracing::debug!("the paste queue is full; a capture was not collected"),
            Err(error) => tracing::warn!(?error, "a capture could not join the paste queue"),
        }
    }

    /// History was cleared; the queue is cleared with it, with no undo.
    pub(crate) fn paste_queue_history_cleared(&self) {
        let mut memory = self.paste_queue.lock();
        memory.undo = None;
        memory.in_flight.clear();
        memory.last_collected = None;
        memory.run_start = None;
        self.deadlines.cancel(&DeadlineKey::new(UNDO_OWNER, ""));
        match self.store.paste_queue_clear() {
            Ok((_, revision)) => self.note_paste_queue_changed(revision),
            Err(error) => tracing::warn!(?error, "the paste queue could not be cleared"),
        }
    }

    /// The rendered text of a snippet and whether it is concealed. The snippet
    /// service renders here once it lands; until then snippets are refused.
    fn render_queued_snippet(
        &self,
        _snippet_id: &str,
        _values: &BTreeMap<String, String>,
    ) -> Option<(Zeroizing<String>, bool)> {
        None
    }

    fn arm_idle_stop(&self, minutes: u32, now: i64) {
        let key = DeadlineKey::new(IDLE_OWNER, "");
        if minutes == 0 {
            self.deadlines.cancel(&key);
            return;
        }
        self.deadlines.schedule(
            key,
            now.saturating_add(i64::from(minutes) * 60_000),
            Box::new(|runtime: &Runtime| runtime.paste_queue_idle_stop()),
        );
    }

    fn paste_queue_idle_stop(&self) {
        let mut memory = self.paste_queue.lock();
        if std::mem::take(&mut memory.collecting) {
            tracing::debug!("paste queue collecting stopped after the idle period");
            self.touch_queue();
        }
    }

    fn paste_queue_undo_expired(&self) {
        let mut memory = self.paste_queue.lock();
        let now = self.deadlines.now();
        if memory.undo.as_ref().is_some_and(|(_, until)| *until <= now) {
            memory.undo = None;
            self.touch_queue();
        }
    }

    /// Move the revision for a change held in memory and announce it.
    fn touch_queue(&self) {
        match self.store.paste_queue_touch(None) {
            Ok(revision) => self.note_paste_queue_changed(revision),
            Err(error) => tracing::warn!(?error, "the paste queue revision could not move"),
        }
    }

    fn stop_collecting(&self, memory: &mut Memory) {
        memory.collecting = false;
        self.deadlines.cancel(&DeadlineKey::new(IDLE_OWNER, ""));
    }

    fn set_collecting(
        &self,
        id: u64,
        memory: &mut Memory,
        on: bool,
        on_finished: QueueRestart,
    ) -> Result<Option<u64>, Failure> {
        if on == memory.collecting {
            return Ok(None);
        }
        if !on {
            self.stop_collecting(memory);
            return self
                .store
                .paste_queue_touch(None)
                .map(Some)
                .map_err(|e| failed(id, &e));
        }
        let settings = self.settings.get().paste_queue.clone();
        if !settings.enabled {
            return Err(refuse(id, ErrorCode::InvalidRequest, MSG_DISABLED));
        }
        let entries = self
            .store
            .paste_queue_entries()
            .map_err(|e| failed(id, &e))?;
        if phase_of(&entries) == QueuePhase::Finished && on_finished == QueueRestart::StartFresh {
            self.store
                .paste_queue_remove_finished()
                .map_err(|e| failed(id, &e))?;
        }
        memory.collecting = true;
        memory.last_collected = None;
        let now = self.deadlines.now();
        self.arm_idle_stop(settings.idle_stop_minutes, now);
        self.store
            .paste_queue_touch(Some(now))
            .map(Some)
            .map_err(|e| failed(id, &e))
    }

    fn add_sources(&self, id: u64, sources: Vec<QueueSource>) -> Result<Option<u64>, Failure> {
        if sources.is_empty() {
            return Ok(None);
        }
        let settings = self.settings.get().paste_queue.clone();
        if !settings.enabled {
            return Err(refuse(id, ErrorCode::InvalidRequest, MSG_DISABLED));
        }
        let queued = self
            .store
            .paste_queue_entries()
            .map_err(|e| failed(id, &e))?
            .len();
        if queued.saturating_add(sources.len()) > settings.max_entries as usize {
            return Err(refuse(id, ErrorCode::InvalidRequest, MSG_FULL));
        }
        let mut added = Vec::with_capacity(sources.len());
        for source in sources {
            added.push(self.new_entry(id, source)?);
        }
        let now = self.deadlines.now();
        match self.store.paste_queue_add(
            &self.keyring.item_key(),
            &added,
            settings.insert_at_start,
            settings.max_entries,
            now,
            false,
        ) {
            Ok(Some((_, revision))) => Ok(Some(revision)),
            Ok(None) => Err(refuse(id, ErrorCode::InvalidRequest, MSG_FULL)),
            Err(error) => Err(failed(id, &error)),
        }
    }

    fn new_entry(&self, id: u64, source: QueueSource) -> Result<NewQueueEntry, Failure> {
        let (clip_id, added_via) = match source {
            QueueSource::Clip { id: clip_id } => (clip_id, QueueAddedVia::HistoryAdd),
            QueueSource::Snippet {
                id: snippet_id,
                values,
            } => {
                let Some((text, concealed)) = self.render_queued_snippet(&snippet_id, &values)
                else {
                    return Err(refuse(id, ErrorCode::Unsupported, MSG_SNIPPETS));
                };
                return Ok(NewQueueEntry {
                    source: if concealed {
                        QueueSourceKind::ConcealedSnippet
                    } else {
                        QueueSourceKind::Snippet
                    },
                    clip_id: None,
                    snapshot: Some(text),
                    content_kind: QueueContentKind::Text,
                    added_via: QueueAddedVia::SnippetAdd,
                    rendered_at: Some(self.deadlines.now()),
                });
            }
            QueueSource::Text { text, via } => {
                if text.trim().is_empty() {
                    return Err(refuse(id, ErrorCode::InvalidRequest, MSG_EMPTY_CONTENT));
                }
                ADDING.with(|adding| adding.set(true));
                let captured = self.capture_text_with_policy(&text, true);
                ADDING.with(|adding| adding.set(false));
                let clip_id =
                    captured.map_err(|_| refuse(id, ErrorCode::InvalidRequest, MSG_NOT_STORED))?;
                let via = match via {
                    QueueTextVia::SelectionAction => QueueAddedVia::SelectionAction,
                    QueueTextVia::Share => QueueAddedVia::Share,
                };
                (clip_id, via)
            }
        };
        let (_, content_type) = self
            .store
            .paste_queue_clip(&clip_id)
            .map_err(|e| failed(id, &e))?
            .ok_or_else(|| refuse(id, ErrorCode::NotFound, MSG_NOT_FOUND))?;
        Ok(NewQueueEntry {
            source: QueueSourceKind::Clip,
            clip_id: Some(clip_id),
            snapshot: None,
            content_kind: content_kind_of(&content_type),
            added_via,
            rendered_at: None,
        })
    }

    fn clear_queue(&self, id: u64, memory: &mut Memory) -> Result<Option<u64>, Failure> {
        let (cleared, revision) = self.store.paste_queue_clear().map_err(|e| failed(id, &e))?;
        memory.in_flight.clear();
        memory.run_start = None;
        let key = DeadlineKey::new(UNDO_OWNER, "");
        if cleared.is_empty() {
            memory.undo = None;
            self.deadlines.cancel(&key);
            return Ok(Some(revision));
        }
        let until = self.deadlines.now().saturating_add(UNDO_MS);
        memory.undo = Some((cleared, until));
        self.deadlines.schedule(
            key,
            until,
            Box::new(|runtime: &Runtime| runtime.paste_queue_undo_expired()),
        );
        Ok(Some(revision))
    }

    fn undo_clear(&self, id: u64, memory: &mut Memory) -> Result<Option<u64>, Failure> {
        let now = self.deadlines.now();
        let Some((cleared, _)) = memory.undo.take().filter(|(_, until)| *until > now) else {
            return Err(refuse(id, ErrorCode::NotFound, MSG_NO_UNDO));
        };
        self.deadlines.cancel(&DeadlineKey::new(UNDO_OWNER, ""));
        self.store
            .paste_queue_restore(&cleared)
            .map(Some)
            .map_err(|e| failed(id, &e))
    }

    fn edit_entry(
        &self,
        id: u64,
        entry_id: &str,
        text: Option<String>,
    ) -> Result<Option<u64>, Failure> {
        let text = text.map(Zeroizing::new);
        if let Some(text) = &text {
            if text.is_empty() {
                return Err(refuse(id, ErrorCode::InvalidRequest, MSG_EMPTY_CONTENT));
            }
            if text.len() > MAX_TEXT_BYTES {
                return Err(refuse(
                    id,
                    ErrorCode::ContentTooLarge,
                    crate::messages::MSG_CONTENT_TOO_LARGE,
                ));
            }
        }
        match self.store.paste_queue_set_override(
            &self.keyring.item_key(),
            entry_id,
            text.as_deref().map(String::as_str),
        ) {
            Ok(Some(revision)) => Ok(Some(revision)),
            Ok(None) => Err(refuse(id, ErrorCode::InvalidRequest, MSG_NOT_TEXT)),
            Err(error) => Err(failed(id, &error)),
        }
    }

    fn skip(&self, id: u64, memory: &Memory) -> Result<Option<u64>, Failure> {
        let reserved = reserved(memory);
        let entries = self
            .store
            .paste_queue_entries()
            .map_err(|e| failed(id, &e))?;
        let Some(head) = entries.iter().find(|entry| {
            entry.status == QueueEntryStatus::Pending && !reserved.contains(&entry.entry_id)
        }) else {
            return Ok(None);
        };
        self.store
            .paste_queue_set_status(
                std::slice::from_ref(&head.entry_id),
                QueueEntryStatus::Skipped,
                None,
            )
            .map(Some)
            .map_err(|e| failed(id, &e))
    }

    fn confirm(
        &self,
        id: u64,
        memory: &mut Memory,
        token: &str,
        outcome: QueueDeliveryOutcome,
    ) -> Result<Option<u64>, Failure> {
        let Some(index) = memory.in_flight.iter().position(|f| f.token == token) else {
            return Err(refuse(id, ErrorCode::NotFound, MSG_UNKNOWN_DELIVERY));
        };
        let Some(flight) = memory.in_flight.remove(index) else {
            return Ok(None);
        };
        match (outcome, flight.awaiting) {
            (QueueDeliveryOutcome::Inserted, true) => {
                let revision = self
                    .store
                    .paste_queue_set_status(
                        &flight.entry_ids,
                        QueueEntryStatus::Pasted,
                        Some(self.deadlines.now()),
                    )
                    .map_err(|e| failed(id, &e))?;
                self.after_pasted(memory, id)?;
                Ok(Some(revision))
            }
            (QueueDeliveryOutcome::Inserted, false) => {
                if let Some(restore) = flight.restore {
                    self.schedule_clipboard_restore(restore, FINISH_RESTORE_MS);
                }
                Ok(None)
            }
            (QueueDeliveryOutcome::Rejected, true) => Ok(None),
            (QueueDeliveryOutcome::Rejected, false) => self
                .store
                .paste_queue_set_status(&flight.entry_ids, QueueEntryStatus::Pending, None)
                .map(Some)
                .map_err(|e| failed(id, &e)),
        }
    }

    /// Entries were just marked pasted. A queue that stops at its end and has
    /// nothing left is finished: collecting turns off.
    fn after_pasted(&self, memory: &mut Memory, id: u64) -> Result<bool, Failure> {
        let settings = self.settings.get().paste_queue.clone();
        if settings.end_behavior != QueueEndBehavior::Stop {
            return Ok(false);
        }
        let entries = self
            .store
            .paste_queue_entries()
            .map_err(|e| failed(id, &e))?;
        let finished = !entries
            .iter()
            .any(|entry| entry.status == QueueEntryStatus::Pending && entry.available);
        if finished {
            self.stop_collecting(memory);
            memory.run_start = None;
        }
        Ok(finished)
    }

    fn deliver_queue(
        &self,
        id: u64,
        memory: &mut Memory,
        target: &QueueTarget,
        plain_text: Option<bool>,
        mode: QueueDeliveryMode,
    ) -> Result<(QueueDelivery, Option<u64>), Failure> {
        let settings = self.settings.get().paste_queue.clone();
        let plain = plain_text.unwrap_or(settings.plain_text);
        let mut revision = None;
        let mut notices = QueueNotices::default();
        let reserved = reserved(memory);
        let mut entries = self
            .store
            .paste_queue_entries()
            .map_err(|e| failed(id, &e))?;
        let deliverable = |entries: &[QueueEntryRow]| -> Vec<QueueEntryRow> {
            entries
                .iter()
                .filter(|entry| {
                    entry.status == QueueEntryStatus::Pending && !reserved.contains(&entry.entry_id)
                })
                .cloned()
                .collect()
        };
        let mut candidates = match target {
            QueueTarget::Entry { id: entry_id } => {
                let entry = entries
                    .iter()
                    .find(|entry| entry.entry_id == *entry_id)
                    .ok_or_else(|| refuse(id, ErrorCode::NotFound, MSG_NOT_FOUND))?;
                vec![entry.clone()]
            }
            QueueTarget::Next | QueueTarget::All => deliverable(&entries),
        };
        if candidates.is_empty() && *target == QueueTarget::Next {
            match settings.end_behavior {
                QueueEndBehavior::Loop if entries.iter().any(finished_entry) => {
                    revision = Some(
                        self.store
                            .paste_queue_repend()
                            .map_err(|e| failed(id, &e))?,
                    );
                    entries = self
                        .store
                        .paste_queue_entries()
                        .map_err(|e| failed(id, &e))?;
                    candidates = deliverable(&entries);
                }
                QueueEndBehavior::KeepLast => {
                    candidates = entries
                        .iter()
                        .filter(|entry| entry.status == QueueEntryStatus::Pasted)
                        .max_by_key(|entry| entry.delivered_at)
                        .cloned()
                        .into_iter()
                        .collect();
                }
                _ => {}
            }
        }
        let mut chosen = Vec::new();
        let mut contents = Vec::new();
        let mut gone = Vec::new();
        let open_text = mode == QueueDeliveryMode::Return || *target == QueueTarget::All;
        for entry in candidates {
            let content = if entry.available {
                self.store
                    .paste_queue_content(
                        &self.keyring.item_key(),
                        &entry.entry_id,
                        plain || mode == QueueDeliveryMode::Return,
                        open_text,
                    )
                    .map_err(|e| failed(id, &e))?
            } else {
                QueueEntryContent::Unavailable
            };
            if matches!(content, QueueEntryContent::Unavailable) {
                gone.push(entry.entry_id.clone());
                continue;
            }
            chosen.push(entry);
            contents.push(content);
            if *target != QueueTarget::All {
                break;
            }
        }
        if !gone.is_empty() {
            notices.unavailable = u32::try_from(gone.len()).unwrap_or(u32::MAX);
            revision = Some(
                self.store
                    .paste_queue_set_status(&gone, QueueEntryStatus::Unavailable, None)
                    .map_err(|e| failed(id, &e))?,
            );
        }
        if chosen.is_empty() {
            let nothing = if entries.iter().any(finished_entry) {
                NothingToPaste::Finished
            } else {
                NothingToPaste::Empty
            };
            return Ok((delivery_of(nothing, notices), revision));
        }
        let kinds: Vec<QueueContentKind> = chosen.iter().map(|entry| entry.content_kind).collect();
        let (text, secret, html) = if *target == QueueTarget::All {
            if kinds.iter().all(|kind| *kind == QueueContentKind::Files) {
                if chosen.len() > 1 {
                    return Err(refuse(id, ErrorCode::Unsupported, MSG_MANY_FILES));
                }
                (None, false, false)
            } else if !kinds.iter().copied().all(is_textual) {
                return Ok((delivery_of(NothingToPaste::Mixed, notices), revision));
            } else {
                let rich = !plain && kinds.iter().all(|kind| *kind == QueueContentKind::RichText);
                let (text, secret, html) = self.join(id, &chosen, contents, &settings, rich)?;
                (Some(text), secret, html)
            }
        } else {
            match contents.pop() {
                Some(QueueEntryContent::Text { text, secret, .. }) => (Some(text), secret, false),
                _ if mode == QueueDeliveryMode::Return => {
                    return Err(refuse(id, ErrorCode::UnsupportedContent, MSG_NOT_TEXT));
                }
                _ => (None, false, false),
            }
        };
        let entry_ids: Vec<String> = chosen.iter().map(|entry| entry.entry_id.clone()).collect();
        let token = next_token(memory);
        if mode == QueueDeliveryMode::Return {
            let finished = settings.end_behavior == QueueEndBehavior::Stop
                && !entries.iter().any(|entry| {
                    entry.status == QueueEntryStatus::Pending
                        && entry.available
                        && !entry_ids.contains(&entry.entry_id)
                });
            remember(
                memory,
                InFlight {
                    token: token.clone(),
                    entry_ids: entry_ids.clone(),
                    awaiting: true,
                    restore: None,
                },
            );
            let delivery = QueueDelivery {
                token: Some(token),
                entry_ids,
                text: text.map(|text| text.to_string()),
                notices,
                finished,
                nothing_to_paste: None,
            };
            return Ok((delivery, revision));
        }
        if memory.run_start.is_none() {
            memory.run_start = Some(self.clipboard_item());
        }
        let restore_token = match text {
            Some(text) => {
                self.deliver_text(
                    &text,
                    DeliveryOptions {
                        secret,
                        transient: true,
                        record_in_history: false,
                        html,
                    },
                )
                .map_err(|error| Box::new(error.response(id)))?
                .token
            }
            None => {
                let clip_id = chosen[0].clip_id.clone().unwrap_or_default();
                let plain = plain && is_textual(chosen[0].content_kind);
                self.deliver_item(&clip_id, plain, false)
                    .map_err(|error| Box::new(error.response(id)))?
                    .token
            }
        };
        let now = self.deadlines.now();
        revision = Some(
            self.store
                .paste_queue_set_status(&entry_ids, QueueEntryStatus::Pasted, Some(now))
                .map_err(|e| failed(id, &e))?,
        );
        let run_start = memory.run_start.clone().flatten();
        let finished = self.after_pasted(memory, id)?;
        let restore = restore_token
            .filter(|_| finished && settings.restore_when_finished)
            .inspect(|token| self.retarget_restore(token, run_start));
        let token = (mode == QueueDeliveryMode::Clipboard).then(|| {
            remember(
                memory,
                InFlight {
                    token: token.clone(),
                    entry_ids: entry_ids.clone(),
                    awaiting: false,
                    restore,
                },
            );
            token
        });
        tracing::debug!(entries = entry_ids.len(), finished, "paste queue delivered");
        let delivery = QueueDelivery {
            token,
            entry_ids,
            text: None,
            notices,
            finished,
            nothing_to_paste: None,
        };
        Ok((delivery, revision))
    }

    /// Join text entries with the separator; rich text stays rich only when
    /// every entry is HTML.
    fn join(
        &self,
        id: u64,
        chosen: &[QueueEntryRow],
        contents: Vec<QueueEntryContent>,
        settings: &PasteQueueSettings,
        rich: bool,
    ) -> Result<(Zeroizing<String>, bool, bool), Failure> {
        let html = rich
            && contents.iter().all(|content| {
                matches!(content, QueueEntryContent::Text { content_type, .. }
                    if content_type == copypaste_ipc::content_type::HTML)
            });
        let contents = if rich && !html {
            let key = self.keyring.item_key();
            chosen
                .iter()
                .map(|entry| {
                    self.store
                        .paste_queue_content(&key, &entry.entry_id, true, true)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| failed(id, &e))?
        } else {
            contents
        };
        let mut separator = separator(settings, CRLF);
        if html {
            separator = html_escape(&separator);
        }
        let mut joined = Zeroizing::new(String::new());
        let mut secret = false;
        for (index, content) in contents.into_iter().enumerate() {
            let QueueEntryContent::Text {
                text,
                secret: entry_secret,
                ..
            } = content
            else {
                return Err(refuse(id, ErrorCode::UnsupportedContent, MSG_NOT_TEXT));
            };
            if index > 0 {
                joined.push_str(&separator);
            }
            joined.push_str(&text);
            secret |= entry_secret;
            if joined.len() > copypaste_ipc::MAX_CONTENT_BYTES {
                return Err(refuse(
                    id,
                    ErrorCode::ContentTooLarge,
                    crate::messages::MSG_CONTENT_TOO_LARGE,
                ));
            }
        }
        Ok((joined, secret, html))
    }

    fn queue_state(&self, memory: &Memory) -> Result<PasteQueueState, StoreError> {
        let settings = self.settings.get().paste_queue.clone();
        let revision = self.store.paste_queue_revision()?;
        let listed = self
            .store
            .paste_queue_previews(&self.keyring.item_key(), &self.keyring.preview_key())?;
        let rows: Vec<_> = listed
            .iter()
            .filter_map(|shown| shown.clip.as_ref().map(|(row, _)| row.clone()))
            .collect();
        let origins = self.meta.origins_for(&rows);
        let mut newest = None;
        let mut pending = false;
        let mut finished = false;
        let mut entries = Vec::with_capacity(listed.len());
        for (index, shown) in listed.into_iter().enumerate() {
            let entry = shown.entry;
            let item = shown.clip.and_then(|(row, preview)| {
                let preview = preview?;
                Some(preview.into_item(&row, origins.get(&row.id)?, self.meta.device_id()))
            });
            let status = if entry.status == QueueEntryStatus::Pending && !entry.available {
                QueueEntryStatus::Unavailable
            } else {
                entry.status
            };
            pending |= status == QueueEntryStatus::Pending;
            finished |= matches!(status, QueueEntryStatus::Pasted | QueueEntryStatus::Skipped);
            newest = newest.max(Some(entry.added_at));
            let masked = entry.source == QueueSourceKind::ConcealedSnippet
                || item.as_ref().is_some_and(|item| item.privacy.secret)
                || shown
                    .annotations
                    .as_ref()
                    .is_some_and(|annotations| annotations.rule_secret);
            let content_kind = match (&item, entry.content_kind) {
                (Some(item), QueueContentKind::Text)
                    if item.semantic_kind == Some(SemanticKind::Link) =>
                {
                    QueueContentKind::Link
                }
                (_, kind) => kind,
            };
            let preview = if masked {
                String::new()
            } else {
                shown
                    .text
                    .or_else(|| item.as_ref().map(|item| item.content.clone()))
                    .unwrap_or_default()
            };
            entries.push(QueueEntry {
                entry_id: entry.entry_id,
                position_label: (index + 1).to_string(),
                status,
                content_kind,
                preview,
                masked,
                edited: entry.edited,
                source_app_bundle_id: item.as_ref().and_then(|i| i.source_app_bundle_id.clone()),
                source_app_name: item.as_ref().and_then(|i| i.source_app_name.clone()),
                source_app_icon_id: item.as_ref().and_then(|i| i.source_app_icon_id.clone()),
                added_via: entry.added_via,
                rendered_at: entry.rendered_at,
            });
        }
        let now = self.deadlines.now();
        let phase = if entries.is_empty() {
            QueuePhase::Empty
        } else if pending {
            QueuePhase::Active
        } else if finished {
            QueuePhase::Finished
        } else {
            QueuePhase::Empty
        };
        Ok(PasteQueueState {
            revision,
            collecting: memory.collecting,
            phase,
            full: entries.len() >= settings.max_entries as usize,
            stale_since: newest.filter(|at| pending && now.saturating_sub(*at) > STALE_MS),
            undo_until: memory
                .undo
                .as_ref()
                .map(|(_, until)| *until)
                .filter(|until| *until > now),
            entries,
            settings,
        })
    }
}

fn finished_entry(entry: &QueueEntryRow) -> bool {
    matches!(
        entry.status,
        QueueEntryStatus::Pasted | QueueEntryStatus::Skipped
    )
}

fn phase_of(entries: &[QueueEntryRow]) -> QueuePhase {
    if entries
        .iter()
        .any(|entry| entry.status == QueueEntryStatus::Pending && entry.available)
    {
        QueuePhase::Active
    } else if entries.iter().any(finished_entry) {
        QueuePhase::Finished
    } else {
        QueuePhase::Empty
    }
}

/// Entries an unconfirmed editor commit holds; no other delivery takes them.
fn reserved(memory: &Memory) -> HashSet<String> {
    memory
        .in_flight
        .iter()
        .filter(|flight| flight.awaiting)
        .flat_map(|flight| flight.entry_ids.iter().cloned())
        .collect()
}

fn remember(memory: &mut Memory, flight: InFlight) {
    if memory.in_flight.len() >= MAX_IN_FLIGHT {
        memory.in_flight.pop_front();
    }
    memory.in_flight.push_back(flight);
}

fn next_token(memory: &mut Memory) -> String {
    memory.tokens = memory.tokens.wrapping_add(1);
    format!(
        "pq{:08x}{:016x}",
        memory.tokens,
        RandomState::new().hash_one(memory.tokens)
    )
}

fn failed(id: u64, error: &StoreError) -> Failure {
    Box::new(storage_error(id, "paste_queue", error))
}

fn refuse(id: u64, code: ErrorCode, message: &str) -> Failure {
    Box::new(Response::err(id, code, message))
}

#[cfg(test)]
mod tests;
