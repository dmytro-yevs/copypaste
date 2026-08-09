//! Getting history out of the service and back in: export, import, backup,
//! restore.
//!
//! # The path never crosses the bridge
//!
//! Every one of these needs a file, and a file needs a path. The path is
//! obtained here, from the platform's own panel, and used here; the WebView is
//! given counts and an opaque one-use token, never a path. That is CLAUDE.md
//! rule 4 held structurally rather than by remembering: there is no path field
//! for React to pass or render, so no authored string and no error can end up
//! carrying the user's home directory.
//!
//! It also keeps the ACL surface at zero. The dialog plugin is called from
//! Rust, and Tauri's capability files gate the JS bridge, so nothing in
//! `capabilities/` has to be widened to make these work.
//!
//! # Cancelling is not failing
//!
//! Commands that open a panel return `Option<_>`, and `None` means the user
//! closed it. A dismissed file picker rendered as an error is the shape of bug
//! that teaches people to ignore error toasts.
//!
//! # What the counts are for
//!
//! An export withholds every item the detector flagged unless it is asked twice
//! — the wire default is `false` and the caller has to opt in — and it
//! *counts* what it withheld. A user who is not told believes they exported
//! everything, which they find out at the moment the export was supposed to
//! save them (`P2-tj9s`, `CopyPaste-93yr`). [`ExportReport`] therefore carries
//! all three skip counts, always, including when they are zero.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Mutex;

use copypaste_ipc::{ExportData, ExportItem, ImportData};
use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

use crate::backend::{Backend, BackendError, SelectedBackend};
use crate::commands::document;

type Result<T> = std::result::Result<T, BackendError>;

/// Suggested names. Not shown by this app — the platform's panel shows them,
/// and the user may replace them — so neither is a string the catalogue owns.
const EXPORT_NAME: &str = "copypaste-export.json";
const BACKUP_NAME: &str = "copypaste-backup.cpbak";

const MSG_UNREADABLE: &str = "That file couldn't be read.";
const MSG_NOT_AN_EXPORT: &str = "That file isn't a CopyPaste export.";
const MSG_IMPORT_TOO_LARGE: &str = "That export is too large to import safely on this device.";
const MSG_IMPORT_EXPIRED: &str = "That import is no longer available. Choose the file again.";
const MSG_IMPORT_STATE: &str = "The pending import couldn't be accessed. Choose the file again.";
const MSG_NOT_WRITTEN: &str = "The export couldn't be written.";

/// A file import must fit comfortably in the UI process. The item-count limit
/// in core constrains work only after JSON decoding, so it cannot protect the
/// process from one enormous JSON string.
const MAX_IMPORT_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// What an export contained, and everything it left out.
#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct ExportReport {
    pub exported: u32,
    /// Flagged items withheld because the caller did not ask for them.
    pub skipped_sensitive: u32,
    /// Items that are not representable in the plaintext export.
    pub skipped_non_text: u32,
    pub skipped_undecryptable: u32,
}

impl From<&ExportData> for ExportReport {
    fn from(data: &ExportData) -> Self {
        Self {
            exported: u32::try_from(data.items.len()).unwrap_or(u32::MAX),
            skipped_sensitive: data.skipped_sensitive,
            skipped_non_text: data.skipped_non_text,
            skipped_undecryptable: data.skipped_undecryptable,
        }
    }
}

/// The only import information the WebView needs before confirmation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct ImportPreview {
    token: String,
    item_count: u32,
}

#[derive(Debug)]
struct PendingImport {
    token: String,
    items: Vec<ExportItem>,
}

/// One bounded, parsed import waiting for an explicit confirmation.
#[derive(Debug, Default)]
pub struct PendingImportState {
    pending: Mutex<Option<PendingImport>>,
}

impl PendingImportState {
    fn clear(&self) -> Result<()> {
        *self
            .pending
            .lock()
            .map_err(|_| BackendError::Internal(MSG_IMPORT_STATE.into()))? = None;
        Ok(())
    }

    fn replace(&self, data: ExportData) -> Result<ImportPreview> {
        let token = uuid::Uuid::new_v4().simple().to_string();
        let preview = ImportPreview {
            token: token.clone(),
            item_count: u32::try_from(data.items.len()).unwrap_or(u32::MAX),
        };
        *self
            .pending
            .lock()
            .map_err(|_| BackendError::Internal(MSG_IMPORT_STATE.into()))? = Some(PendingImport {
            token,
            items: data.items,
        });
        Ok(preview)
    }

    fn cancel(&self, token: &str) -> Result<()> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| BackendError::Internal(MSG_IMPORT_STATE.into()))?;
        if pending
            .as_ref()
            .is_some_and(|candidate| candidate.token == token)
        {
            *pending = None;
        }
        Ok(())
    }

    fn take(&self, token: &str) -> Result<Vec<ExportItem>> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| BackendError::Internal(MSG_IMPORT_STATE.into()))?;
        let Some(candidate) = pending.take() else {
            return Err(BackendError::Invalid(MSG_IMPORT_EXPIRED));
        };
        if candidate.token != token {
            *pending = Some(candidate);
            return Err(BackendError::Invalid(MSG_IMPORT_EXPIRED));
        }
        Ok(candidate.items)
    }
}

/// Write history to a file the user picks.
///
/// `include_sensitive` is the second ask. The first is the dialog that offers
/// it; passing `true` puts credentials in a plaintext file that leaves the
/// app's control.
#[tauri::command]
pub async fn export_history<R: Runtime>(
    app: AppHandle<R>,
    backend: State<'_, SelectedBackend>,
    include_sensitive: bool,
) -> Result<Option<ExportReport>> {
    // The read happens before the panel opens, so a build that cannot export
    // says so instead of asking where to put nothing.
    let data = backend.export(0, include_sensitive).await?;

    let Some(dest) = document::save_panel_file(&app, EXPORT_NAME).await else {
        return Ok(None);
    };

    let encoded = serde_json::to_string_pretty(&data)
        .map_err(|_| BackendError::Internal(MSG_NOT_WRITTEN.into()))?;
    document::write_picked(&app, dest, encoded.as_bytes(), MSG_NOT_WRITTEN)?;

    Ok(Some(ExportReport::from(&data)))
}

/// Choose and parse an export without writing anything.
#[tauri::command]
pub async fn prepare_import_history<R: Runtime>(
    app: AppHandle<R>,
    pending: State<'_, PendingImportState>,
) -> Result<Option<ImportPreview>> {
    pending.clear()?;
    let Some(src) = document::open_panel_file(&app).await else {
        return Ok(None);
    };
    let local = document::picked_to_temp(
        &app,
        src,
        MAX_IMPORT_FILE_BYTES,
        MSG_IMPORT_TOO_LARGE,
        MSG_UNREADABLE,
    )?;
    Ok(Some(preview_file(&pending, local.path())?))
}

/// Consume one prepared import and route every item through ordinary ingest.
#[tauri::command]
pub async fn apply_import_history(
    backend: State<'_, SelectedBackend>,
    pending: State<'_, PendingImportState>,
    token: String,
) -> Result<ImportData> {
    let items = pending.take(&token)?;
    backend.import(items).await
}

/// Drop a prepared import. Repeated and stale cancellations are harmless.
#[tauri::command]
pub fn cancel_import_history(pending: State<'_, PendingImportState>, token: String) -> Result<()> {
    pending.cancel(&token)
}

fn preview_file(pending: &PendingImportState, src: &Path) -> Result<ImportPreview> {
    pending.replace(read_export(src)?)
}

fn read_export(src: &Path) -> Result<ExportData> {
    let file = File::open(src).map_err(|_| BackendError::Invalid(MSG_UNREADABLE))?;
    let bytes = file
        .metadata()
        .map_err(|_| BackendError::Invalid(MSG_UNREADABLE))?
        .len();
    if bytes > MAX_IMPORT_FILE_BYTES {
        return Err(BackendError::Invalid(MSG_IMPORT_TOO_LARGE));
    }

    serde_json::from_reader(BufReader::new(file))
        .map_err(|_| BackendError::Invalid(MSG_NOT_AN_EXPORT))
}

/// Copy the encrypted database to a file the user picks.
///
/// The copy is ciphertext and opens only with this device's key, so it is
/// useless on another machine — which is also why it is not an export.
#[tauri::command]
pub async fn backup_database<R: Runtime>(
    app: AppHandle<R>,
    backend: State<'_, SelectedBackend>,
) -> Result<Option<u64>> {
    let Some(dest) = document::save_panel_file(&app, BACKUP_NAME).await else {
        return Ok(None);
    };
    let dir = tempfile::tempdir().map_err(|_| BackendError::Internal(MSG_NOT_WRITTEN.into()))?;
    let local = dir.path().join(BACKUP_NAME);
    let report = backend.backup(&local).await?;
    let mut backup =
        File::open(&local).map_err(|_| BackendError::Internal(MSG_NOT_WRITTEN.into()))?;
    document::copy_to_picked(&app, dest, &mut backup, MSG_NOT_WRITTEN)?;
    Ok(Some(report.size_bytes))
}

/// Replace this device's history with a backup.
///
/// The most destructive thing the product does, and the only confirmation is
/// the caller's: reaching here means the user has already answered a dialog
/// naming what is about to be lost.
///
/// The service validates before it replaces — it stages the file, opens it with
/// this device's real key and checks it — so a damaged or foreign file leaves
/// history exactly as it was.
#[tauri::command]
pub async fn restore_database<R: Runtime>(
    app: AppHandle<R>,
    backend: State<'_, SelectedBackend>,
) -> Result<bool> {
    let Some(src) = document::open_panel_file(&app).await else {
        return Ok(false);
    };
    let local =
        document::picked_to_temp(&app, src, u64::MAX, MSG_IMPORT_TOO_LARGE, MSG_UNREADABLE)?;
    backend.restore(local.path()).await?;
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_ipc::ExportItem;

    fn data(items: usize, skipped_sensitive: u32) -> ExportData {
        ExportData {
            items: (0..items)
                .map(|n| ExportItem {
                    content: format!("clip {n}"),
                    content_type: "text/plain".into(),
                    created_at: 0,
                    pinned: false,
                    is_sensitive: false,
                })
                .collect(),
            skipped_non_text: 1,
            skipped_sensitive,
            skipped_undecryptable: 2,
        }
    }

    /// The count is the whole point: a shorter file and a smaller history are
    /// the same thing to a user who is not told.
    #[test]
    fn the_report_carries_every_skip_count_including_zero() {
        let report = ExportReport::from(&data(3, 0));
        assert_eq!(report.exported, 3);
        assert_eq!(report.skipped_sensitive, 0);
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"skipped_sensitive\":0"), "{json}");
    }

    #[test]
    fn a_withheld_item_is_counted_rather_than_dropped_quietly() {
        assert_eq!(ExportReport::from(&data(1, 4)).skipped_sensitive, 4);
    }

    /// Every sentence this file can show is authored here and holds no path.
    #[test]
    fn no_message_names_a_place() {
        for message in [
            MSG_UNREADABLE,
            MSG_NOT_AN_EXPORT,
            MSG_IMPORT_TOO_LARGE,
            MSG_IMPORT_EXPIRED,
            MSG_IMPORT_STATE,
            MSG_NOT_WRITTEN,
        ] {
            assert!(!message.contains('/'), "{message}");
            assert!(!message.contains('~'), "{message}");
        }
    }

    /// The file an export writes is the file an import reads. One shape, and it
    /// is `copypaste_ipc::ExportData` — the same bytes `copypaste export -o`
    /// produces, so a file moves between the CLI and the window.
    #[test]
    fn an_export_round_trips_through_the_form_it_is_written_in() {
        let encoded = serde_json::to_string_pretty(&data(2, 0)).unwrap();
        let decoded: ExportData = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.items.len(), 2);
        assert_eq!(decoded.skipped_undecryptable, 2);
    }

    #[test]
    fn something_that_is_not_an_export_is_refused_rather_than_half_read() {
        assert!(serde_json::from_str::<ExportData>(r#"{"nope":true}"#).is_err());
    }

    #[test]
    fn an_oversized_import_is_refused_before_json_can_allocate_it() {
        let file = tempfile::NamedTempFile::new().unwrap();
        file.as_file().set_len(MAX_IMPORT_FILE_BYTES + 1).unwrap();

        assert!(matches!(
            read_export(file.path()),
            Err(BackendError::Invalid(MSG_IMPORT_TOO_LARGE))
        ));
    }

    #[test]
    fn an_invalid_file_creates_no_confirmation_or_pending_write() {
        let pending = PendingImportState::default();
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"not json").unwrap();

        pending.clear().unwrap();
        assert!(preview_file(&pending, file.path()).is_err());
        assert!(matches!(
            pending.take("anything"),
            Err(BackendError::Invalid(MSG_IMPORT_EXPIRED))
        ));
    }

    #[test]
    fn a_valid_file_previews_its_item_count_without_exposing_contents() {
        let pending = PendingImportState::default();
        let file = tempfile::NamedTempFile::new().unwrap();
        serde_json::to_writer(file.as_file(), &data(3, 0)).unwrap();

        let preview = preview_file(&pending, file.path()).unwrap();
        assert_eq!(preview.item_count, 3);
        let json = serde_json::to_string(&preview).unwrap();
        assert!(!json.contains("clip 0"), "{json}");
        assert_eq!(pending.take(&preview.token).unwrap().len(), 3);
    }

    #[test]
    fn cancelling_is_idempotent_and_releases_the_pending_import() {
        let pending = PendingImportState::default();
        let preview = pending.replace(data(1, 0)).unwrap();

        pending.cancel(&preview.token).unwrap();
        pending.cancel(&preview.token).unwrap();
        assert!(matches!(
            pending.take(&preview.token),
            Err(BackendError::Invalid(MSG_IMPORT_EXPIRED))
        ));
    }

    #[test]
    fn a_confirmation_token_releases_items_exactly_once() {
        let pending = PendingImportState::default();
        let preview = pending.replace(data(2, 0)).unwrap();

        assert_eq!(pending.take(&preview.token).unwrap().len(), 2);
        assert!(matches!(
            pending.take(&preview.token),
            Err(BackendError::Invalid(MSG_IMPORT_EXPIRED))
        ));
    }

    #[test]
    fn replacing_a_preview_makes_the_old_token_stale() {
        let pending = PendingImportState::default();
        let old = pending.replace(data(1, 0)).unwrap();
        let current = pending.replace(data(2, 0)).unwrap();

        pending.cancel(&old.token).unwrap();
        assert!(matches!(
            pending.take(&old.token),
            Err(BackendError::Invalid(MSG_IMPORT_EXPIRED))
        ));
        assert_eq!(pending.take(&current.token).unwrap().len(), 2);
    }
}
