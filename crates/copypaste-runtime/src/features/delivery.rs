//! Clipboard delivery of produced text and stored items, with restore.
//!
//! One rule for every CopyPaste-originated clipboard write. Copying a stored
//! item is an own write that promotes its row (K33). Any other delivery
//! (rendered, transformed or queued content) is transient: no history row, no
//! promotion, no copy feedback, and the platform's do-not-record markers. A
//! delivery returns a token that can put back what the clipboard held before,
//! and the restore runs only while the clipboard still holds that delivery.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::{Mutex, MutexGuard};

use copypaste_core::history_actions::{self, CopyOptions};
use copypaste_core::history_error::HistoryError;
use copypaste_core::{now_ms, ClipboardPayload, IngestError};
use copypaste_ipc::clips::{ClipOp, Delivered, Delivery};
use copypaste_ipc::{content_type, ErrorCode, Item, Response, ResponseData};

use crate::deadlines::DeadlineKey;
use crate::host::WriteOptions;
use crate::messages::{storage_error, MSG_EMPTY_CONTENT, MSG_ENCRYPT, MSG_TOO_BIG};
use crate::Runtime;

/// The longest delay a caller may ask before a restore.
const MAX_RESTORE_DELAY_MS: u32 = 10 * 60 * 1000;
const RESTORE_OWNER: &str = "clipboard_restore";

/// How [`Runtime::deliver_text`] writes produced text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DeliveryOptions {
    pub(crate) secret: bool,
    /// Marked do-not-record, and never given copy feedback.
    pub(crate) transient: bool,
    /// Store the text as a new history item and copy that item instead.
    pub(crate) record_in_history: bool,
}

/// The outcome of one clipboard delivery.
#[derive(Debug, Clone, Default)]
pub(crate) struct ClipboardDelivery {
    /// Restores the previous clipboard; absent for an ordinary copy.
    pub(crate) token: Option<String>,
    /// The history item the clipboard now holds, when there is one.
    pub(crate) item: Option<Item>,
}

#[derive(Debug)]
pub(crate) enum DeliveryError {
    History(HistoryError),
    Ingest(IngestError),
}

impl From<HistoryError> for DeliveryError {
    fn from(error: HistoryError) -> Self {
        Self::History(error)
    }
}

impl DeliveryError {
    pub(crate) fn response(&self, id: u64) -> Response {
        match self {
            Self::History(error) => error.response(id),
            Self::Ingest(IngestError::Empty) => {
                Response::err(id, ErrorCode::InvalidRequest, MSG_EMPTY_CONTENT)
            }
            Self::Ingest(IngestError::TooLarge) => {
                Response::err(id, ErrorCode::ContentTooLarge, MSG_TOO_BIG)
            }
            Self::Ingest(IngestError::Storage(error)) => storage_error(id, "deliver", error),
            Self::Ingest(error) => {
                tracing::error!(?error, "a delivery could not be recorded");
                Response::err(id, ErrorCode::Internal, MSG_ENCRYPT)
            }
        }
    }
}

/// Who wrote the clipboard last, as far as this runtime observed, and the one
/// restore that may still run.
#[derive(Default)]
pub(crate) struct ClipboardOwner(Mutex<Owner>);

#[derive(Default)]
struct Owner {
    /// Advances on every clipboard change this runtime writes or captures.
    generation: u64,
    /// The history item the clipboard holds, when known.
    item: Option<String>,
    pending: Option<PendingRestore>,
    tokens: u64,
}

struct PendingRestore {
    token: String,
    generation: u64,
    previous: Option<String>,
}

impl ClipboardOwner {
    fn lock(&self) -> MutexGuard<'_, Owner> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The clipboard changed by anything but a restorable delivery: a capture,
    /// a copy, a received item, or a change a host saw without capturing it.
    pub(crate) fn changed(&self, item: Option<&str>) {
        let mut owner = self.lock();
        owner.generation = owner.generation.wrapping_add(1);
        owner.item = item.map(str::to_owned);
    }

    fn current(&self) -> Option<String> {
        self.lock().item.clone()
    }

    /// Record a restorable delivery; replaces any earlier pending restore.
    fn delivered(&self, previous: Option<String>, item: Option<&str>) -> String {
        let mut owner = self.lock();
        owner.generation = owner.generation.wrapping_add(1);
        owner.tokens = owner.tokens.wrapping_add(1);
        owner.item = item.map(str::to_owned);
        let token = format!(
            "{:016x}{:016x}",
            owner.tokens,
            RandomState::new().hash_one(owner.generation)
        );
        owner.pending = Some(PendingRestore {
            token: token.clone(),
            generation: owner.generation,
            previous,
        });
        token
    }

    /// The previous item to put back, if `token` names the pending restore
    /// and nothing has changed the clipboard since.
    fn take_restore(&self, token: &str) -> Option<Option<String>> {
        let mut owner = self.lock();
        let pending = owner.pending.take_if(|pending| pending.token == token)?;
        (pending.generation == owner.generation).then_some(pending.previous)
    }
}

impl Runtime {
    /// Copy a stored item. With `promote` it is an own write of a stored item:
    /// the row moves to the top and copy feedback plays. Without, it is a
    /// transient delivery that leaves history untouched and can be restored.
    pub(crate) fn deliver_item(
        &self,
        item_id: &str,
        plain_text: bool,
        promote: bool,
    ) -> Result<ClipboardDelivery, HistoryError> {
        let key = self.keyring.item_key();
        let opened = history_actions::open(&self.store, &key, item_id)?;
        let origin = self.meta.origin_of(opened.row());
        let prepared = history_actions::prepare_copy(
            &self.store,
            &key,
            opened,
            CopyOptions {
                origin: &origin,
                local_device_id: self.meta.device_id(),
                plain_text,
                allow_content_uri: self.host.writes_content_uris(),
                max_file_size_bytes: self.settings.get().max_file_size_bytes,
            },
        )?;
        let secret = prepared.item.privacy.secret;
        let previous = self.changes.clipboard.current();
        self.host.write(
            &prepared.item.id,
            prepared.payload(),
            &prepared.content_type,
            WriteOptions {
                secret,
                transient: !promote,
            },
        )?;
        if !promote {
            let token = self.changes.clipboard.delivered(previous, Some(item_id));
            return Ok(ClipboardDelivery {
                token: Some(token),
                item: Some(prepared.item),
            });
        }
        self.host.copy_feedback();
        let item = history_actions::finish_copy(&self.store, prepared)?;
        self.changes.clipboard.changed(Some(&item.id));
        self.instant_clipboard
            .note_local(&self.store, item.created_at);
        self.note_local_change_since(item.created_at);
        Ok(ClipboardDelivery {
            token: None,
            item: Some(item),
        })
    }

    /// Put produced text on the clipboard. Transient text creates no history
    /// row; recorded text becomes a new item that is then copied like one.
    pub(crate) fn deliver_text(
        &self,
        text: &str,
        options: DeliveryOptions,
    ) -> Result<ClipboardDelivery, DeliveryError> {
        if text.len() > copypaste_ipc::MAX_CONTENT_BYTES {
            return Err(HistoryError::ContentTooLarge.into());
        }
        let previous = self.changes.clipboard.current();
        if options.record_in_history {
            let item_id = self.record_text(text, options.secret)?;
            let copied = self.deliver_item(&item_id, false, true)?;
            return Ok(ClipboardDelivery {
                token: Some(self.changes.clipboard.delivered(previous, Some(&item_id))),
                item: copied.item,
            });
        }
        self.host
            .write(
                "",
                &ClipboardPayload::Text(text.to_owned().into()),
                content_type::TEXT,
                WriteOptions {
                    secret: options.secret,
                    transient: options.transient,
                },
            )
            .map_err(HistoryError::from)?;
        if !options.transient {
            self.host.copy_feedback();
        }
        Ok(ClipboardDelivery {
            token: Some(self.changes.clipboard.delivered(previous, None)),
            item: None,
        })
    }

    fn record_text(&self, text: &str, secret: bool) -> Result<String, DeliveryError> {
        let settings = self.settings.config();
        let metadata = secret.then(|| copypaste_core::PayloadMetadata {
            privacy: copypaste_ipc::ClipboardPrivacy {
                secret: true,
                transient: false,
            },
            ..Default::default()
        });
        copypaste_core::ingest_into_with_capture_source_metadata_with_current_retention(
            &self.store,
            &self.keyring,
            text,
            content_type::TEXT,
            now_ms(),
            None,
            None,
            metadata.as_ref(),
            &settings,
            || self.settings.config(),
        )
        .map(|ingested| ingested.into_item().id)
        .map_err(DeliveryError::Ingest)
    }

    /// Put back what the clipboard held before `token`'s delivery, unless the
    /// clipboard changed since. A previous value that is unknown or gone
    /// clears the clipboard instead. Answers whether anything was written.
    pub(crate) fn restore_clipboard(&self, token: &str) -> bool {
        let Some(previous) = self.changes.clipboard.take_restore(token) else {
            return false;
        };
        let key = self.keyring.item_key();
        let restored = previous.as_deref().and_then(|item_id| {
            let opened = history_actions::open(&self.store, &key, item_id).ok()?;
            let origin = self.meta.origin_of(opened.row());
            history_actions::prepare_copy(
                &self.store,
                &key,
                opened,
                CopyOptions {
                    origin: &origin,
                    local_device_id: self.meta.device_id(),
                    plain_text: false,
                    allow_content_uri: self.host.writes_content_uris(),
                    max_file_size_bytes: self.settings.get().max_file_size_bytes,
                },
            )
            .ok()
        });
        let written = match &restored {
            Some(prepared) => self.host.write(
                &prepared.item.id,
                prepared.payload(),
                &prepared.content_type,
                WriteOptions {
                    secret: prepared.item.privacy.secret,
                    transient: true,
                },
            ),
            None => self.host.write(
                "",
                &ClipboardPayload::Text(String::new().into()),
                content_type::TEXT,
                WriteOptions {
                    secret: false,
                    transient: true,
                },
            ),
        };
        if let Err(error) = written {
            tracing::warn!(?error, "the clipboard could not be restored");
            return false;
        }
        self.changes
            .clipboard
            .changed(restored.as_ref().map(|prepared| prepared.item.id.as_str()));
        true
    }

    /// Restore after `after_ms`, through the deadline scheduler. One restore
    /// is pending at a time; a later request replaces an earlier one.
    pub(crate) fn schedule_clipboard_restore(&self, token: String, after_ms: u32) {
        if after_ms == 0 {
            self.restore_clipboard(&token);
            return;
        }
        let at = self
            .deadlines
            .now()
            .saturating_add(i64::from(after_ms.min(MAX_RESTORE_DELAY_MS)));
        self.deadlines.schedule(
            DeadlineKey::new(RESTORE_OWNER, ""),
            at,
            Box::new(move |runtime: &Runtime| {
                runtime.restore_clipboard(&token);
            }),
        );
    }
}

pub(super) fn handle(runtime: &Runtime, id: u64, op: ClipOp) -> Response {
    match op {
        ClipOp::DeliverText {
            text,
            secret,
            delivery,
        } => {
            if delivery == Delivery::Return {
                return Response::ok(
                    id,
                    ResponseData::Delivered(Delivered {
                        text: Some(text),
                        ..Delivered::default()
                    }),
                );
            }
            match runtime.deliver_text(
                &text,
                DeliveryOptions {
                    secret,
                    transient: true,
                    record_in_history: false,
                },
            ) {
                Ok(delivered) => Response::ok(
                    id,
                    ResponseData::Delivered(Delivered {
                        token: delivered.token,
                        ..Delivered::default()
                    }),
                ),
                Err(error) => error.response(id),
            }
        }
        _ => crate::features::unsupported(id),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use copypaste_core::{ClipboardWriteError, Keyring};
    use copypaste_ipc::{EventKind, Method};

    use super::*;
    use crate::host::{ClipboardWriter, InProcessHost};

    #[derive(Default)]
    struct Recording(Mutex<Vec<(String, WriteOptions)>>);

    impl ClipboardWriter for Recording {
        fn write(
            &self,
            payload: &ClipboardPayload,
            _: &str,
            options: WriteOptions,
        ) -> Result<(), ClipboardWriteError> {
            let text = match payload {
                ClipboardPayload::Text(text) => text.as_str().to_owned(),
                _ => String::from("<binary>"),
            };
            self.0.lock().unwrap().push((text, options));
            Ok(())
        }
    }

    fn runtime() -> (Arc<Runtime>, Arc<Recording>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let clipboard = Arc::new(Recording::default());
        let runtime = Runtime::open_with_keyring(
            dir.path(),
            "delivery",
            0,
            Arc::new(InProcessHost(clipboard.clone())),
            Arc::new(Keyring::from_secret(&[7; 32])),
        )
        .unwrap();
        (Arc::new(runtime), clipboard, dir)
    }

    fn deliver(runtime: &Runtime, text: &str) -> String {
        let response = runtime.request_blocking(
            1,
            Method::Clips {
                op: ClipOp::DeliverText {
                    text: text.into(),
                    secret: false,
                    delivery: Delivery::Paste,
                },
            },
        );
        match response.data {
            Some(ResponseData::Delivered(delivered)) => delivered.token.unwrap(),
            other => panic!("unexpected {other:?}"),
        }
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

    #[test]
    fn a_transient_delivery_adds_no_history_row_and_emits_no_change() {
        let (runtime, clipboard, _dir) = runtime();
        capture(&runtime, "kept");
        let mut events = runtime.subscribe_events();
        let before = runtime.store.count().unwrap();
        deliver(&runtime, "rendered output");
        assert_eq!(runtime.store.count().unwrap(), before);
        assert!(events.try_recv().is_err());
        let writes = clipboard.0.lock().unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0, "rendered output");
        assert!(writes[0].1.transient && !writes[0].1.secret);
    }

    #[test]
    fn a_restore_puts_back_the_previous_item_only_while_the_delivery_is_current() {
        let (runtime, clipboard, _dir) = runtime();
        capture(&runtime, "previous clip");
        let token = deliver(&runtime, "pasted text");
        assert!(runtime.restore_clipboard(&token));
        assert!(!runtime.restore_clipboard(&token), "a token restores once");
        assert_eq!(
            clipboard.0.lock().unwrap().last().unwrap().0,
            "previous clip"
        );

        let token = deliver(&runtime, "pasted again");
        capture(&runtime, "a later copy");
        let writes = clipboard.0.lock().unwrap().len();
        assert!(!runtime.restore_clipboard(&token));
        assert_eq!(clipboard.0.lock().unwrap().len(), writes);
    }

    #[test]
    fn copying_a_stored_item_promotes_it_and_is_not_transient() {
        let (runtime, clipboard, _dir) = runtime();
        let older = capture(&runtime, "older");
        capture(&runtime, "newer");
        let mut events = runtime.subscribe_events();
        let delivered = runtime.deliver_item(&older, false, true).unwrap();
        assert!(delivered.token.is_none());
        assert_eq!(runtime.store.list(10, 0).unwrap()[0].id, older);
        assert!(!clipboard.0.lock().unwrap()[0].1.transient);
        assert_eq!(events.try_recv().unwrap().event, EventKind::Items);
    }
}
