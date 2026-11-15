//! Expiring clips: setting an expiry, keeping a clip past it, and the purge
//! every deadline sweep runs.

use copypaste_core::storage::expiry::{Clocks, KeepOutcome};
use copypaste_core::{history_actions, now_ms, ClipboardPayload, StoreError};
use copypaste_ipc::clips::ClipOp;
use copypaste_ipc::{content_type, ErrorCode, Response, ResponseData};

use crate::host::WriteOptions;
use crate::messages::{decrypt_error, storage_error, MSG_NOT_FOUND};
use crate::Runtime;

pub(super) fn handle(runtime: &Runtime, id: u64, op: ClipOp) -> Response {
    let ClipOp::Keep { id: clip_id } = op else {
        return crate::features::unsupported(id);
    };
    match runtime.keep_clip(&clip_id, false) {
        Ok(Some(_)) => kept_item(runtime, id, &clip_id),
        Ok(None) => Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(error) => storage_error(id, "keep", &error),
    }
}

/// The kept clip with its annotations, its body bounded like any mutation's.
fn kept_item(runtime: &Runtime, id: u64, clip_id: &str) -> Response {
    let row = match runtime.store.get(clip_id) {
        Ok(Some(row)) => row,
        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(error) => return storage_error(id, "get", &error),
    };
    match runtime.to_wire(row) {
        Ok((mut item, payload)) => {
            if history_actions::oversized_text(&payload) {
                item.truncated = copypaste_ipc::limits::bound_preview(&mut item.content);
            }
            item.annotations = runtime
                .store
                .item_annotations(&[item.id.clone()])
                .ok()
                .and_then(|mut found| found.remove(&item.id));
            Response::ok(id, ResponseData::Item(item))
        }
        Err(error) => decrypt_error(id, &error),
    }
}

impl Runtime {
    /// Expire a clip `duration_ms` from now, as the device that purges it with
    /// a synced tombstone. Answers the deadline, or `None` when the clip is
    /// gone, pinned or kept.
    pub fn set_clip_expiry(
        &self,
        clip_id: &str,
        duration_ms: i64,
        clear_clipboard: bool,
    ) -> Result<Option<i64>, StoreError> {
        let started = now_ms();
        let at = self.store.set_clip_expiry(
            &self.keyring.item_key(),
            clip_id,
            duration_ms,
            clear_clipboard,
            Clocks::at(self.deadlines.now()),
        )?;
        if let Some(at) = at {
            self.note_store_deadline(at);
            self.note_local_change_since(started);
        }
        Ok(at)
    }

    /// Keep a clip: its expiry is cleared on every device and a clip that was
    /// withheld from cloud is offered to it again. With `only_if_expiring` a
    /// clip without an expiry is left alone. `None` when the clip is gone.
    pub(crate) fn keep_clip(
        &self,
        clip_id: &str,
        only_if_expiring: bool,
    ) -> Result<Option<KeepOutcome>, StoreError> {
        let started = now_ms();
        let outcome = self
            .store
            .keep_clip(&self.keyring.item_key(), clip_id, only_if_expiring)?;
        if let Some(outcome) = &outcome {
            if !outcome.reoffer.is_empty() {
                self.modules.reoffer(&outcome.reoffer);
            }
            if outcome.kept {
                self.note_local_change_since(started);
            }
        }
        Ok(outcome)
    }

    /// Pinning an expiring clip keeps it.
    pub(crate) fn keep_pinned_clip(&self, clip_id: &str) {
        if let Err(error) = self.keep_clip(clip_id, true) {
            tracing::warn!(?error, "a pinned clip kept its expiry");
        }
    }

    /// Purge the clips whose expiry passed by `now_ms`. Every deadline sweep
    /// runs it; read-time filtering already hides them until it does.
    pub(crate) fn expire_due_clips(&self, now_ms: i64) {
        let started = copypaste_core::now_ms();
        let swept = match self.store.expire_due(Clocks::at(now_ms)) {
            Ok(swept) => swept,
            Err(error) => {
                tracing::warn!(?error, "expired clips could not be purged");
                return;
            }
        };
        let clipboard = self.clipboard_item();
        if clipboard
            .as_deref()
            .is_some_and(|held| swept.clear_clipboard.iter().any(|id| id == held))
        {
            self.clear_clipboard();
        }
        if !swept.tombstoned.is_empty() {
            self.note_local_change_since(started);
        } else if swept.purged_locally > 0 {
            self.note_remote_change();
        }
        if !swept.tombstoned.is_empty() || swept.purged_locally > 0 {
            tracing::debug!(
                tombstoned = swept.tombstoned.len(),
                purged_locally = swept.purged_locally,
                "expired clips purged"
            );
        }
    }

    fn clear_clipboard(&self) {
        let cleared = self.host.write(
            "",
            &ClipboardPayload::Text(String::new().into()),
            content_type::TEXT,
            WriteOptions {
                secret: false,
                transient: true,
            },
        );
        match cleared {
            Ok(()) => self.changes.clipboard.changed(None),
            Err(error) => tracing::warn!(?error, "the clipboard could not be cleared at expiry"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use copypaste_core::{ClipboardWriteError, Keyring};
    use copypaste_ipc::{Item, Method};

    use super::*;
    use crate::host::{ClipboardWriter, InProcessHost};

    const MIN: i64 = 60 * 1000;

    #[derive(Default)]
    struct Recording(Mutex<Vec<String>>);

    impl ClipboardWriter for Recording {
        fn write(
            &self,
            payload: &ClipboardPayload,
            _: &str,
            _: WriteOptions,
        ) -> Result<(), ClipboardWriteError> {
            if let ClipboardPayload::Text(text) = payload {
                self.0.lock().unwrap().push(text.as_str().to_owned());
            }
            Ok(())
        }
    }

    fn runtime() -> (Arc<Runtime>, Arc<Recording>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let clipboard = Arc::new(Recording::default());
        let runtime = Runtime::open_with_keyring(
            dir.path(),
            "expiry",
            0,
            Arc::new(InProcessHost(clipboard.clone())),
            Arc::new(Keyring::from_secret(&[7; 32])),
        )
        .unwrap();
        (Arc::new(runtime), clipboard, dir)
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

    fn item(response: Response) -> Item {
        match response.data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn a_wake_sweep_tombstones_and_clears_the_clipboard_only_while_it_holds_the_clip() {
        let (runtime, clipboard, _dir) = runtime();
        let replaced = capture(&runtime, "copied over later");
        runtime.set_clip_expiry(&replaced, 5 * MIN, true).unwrap();
        let held = capture(&runtime, "still on the clipboard");
        runtime.set_clip_expiry(&held, 5 * MIN, true).unwrap();
        runtime.deadlines.advance(6 * MIN);
        runtime.sweep_deadlines();
        for id in [&replaced, &held] {
            assert!(runtime.store.version(id).unwrap().unwrap().deleted);
        }
        assert_eq!(*clipboard.0.lock().unwrap(), vec![String::new()]);

        let later = capture(&runtime, "expiring");
        runtime.set_clip_expiry(&later, 5 * MIN, true).unwrap();
        capture(&runtime, "copied after it");
        runtime.deadlines.advance(6 * MIN);
        runtime.sweep_deadlines();
        assert!(runtime.store.version(&later).unwrap().unwrap().deleted);
        assert_eq!(clipboard.0.lock().unwrap().len(), 1, "a later copy stays");
    }

    #[test]
    fn keep_and_pin_clear_the_expiry() {
        let (runtime, _clipboard, _dir) = runtime();
        let kept = capture(&runtime, "keep");
        let pinned = capture(&runtime, "pin");
        for id in [&kept, &pinned] {
            runtime.set_clip_expiry(id, 5 * MIN, false).unwrap();
        }
        let response = runtime.request_blocking(
            1,
            Method::Clips {
                op: ClipOp::Keep { id: kept.clone() },
            },
        );
        let annotations = item(response).annotations.unwrap();
        assert!(annotations.keep && annotations.expires_at.is_none());

        for (request, pin) in [(2, true), (3, false)] {
            runtime.request_blocking(
                request,
                Method::Pin {
                    id: pinned.clone(),
                    pinned: pin,
                },
            );
        }
        let found = runtime
            .store
            .item_annotations(std::slice::from_ref(&pinned))
            .unwrap();
        assert!(found[&pinned].keep && found[&pinned].expires_at.is_none());

        runtime.deadlines.advance(6 * MIN);
        runtime.sweep_deadlines();
        for id in [&kept, &pinned] {
            assert!(runtime.store.get(id).unwrap().is_some());
        }
    }
}
