//! Export and import, in-process.
//!
//! Both are `copypaste_core::transfer` — the same two functions the daemon's
//! IPC handlers call. What is here is only the mapping onto [`BackendError`],
//! which is what a user reads.

use copypaste_ipc::{ExportData, ExportItem, ImportData};

use super::open::Inner;
use super::{BackendError, Result};

const MSG_EXPORT_FAILED: &str = "Your history couldn't be read.";
const MSG_IMPORT_EMPTY: &str = "That file has nothing in it to import.";
const MSG_IMPORT_TOO_MANY: &str =
    "That file holds too many items to import at once. Split it into smaller files.";
const MSG_IMPORT_FAILED: &str = "That file couldn't be imported.";

/// The withheld count comes back with the items and is not recomputed here: a
/// user who is not told believes they exported everything, and a second
/// accounting is how the two platforms would come to disagree about what a short
/// export means (AGENTS.md rule 1).
pub(super) fn export(inner: &Inner, limit: u32, include_sensitive: bool) -> Result<ExportData> {
    copypaste_core::export(
        &inner.state.store,
        &inner.state.keyring,
        limit,
        include_sensitive,
    )
    .map_err(|e| {
        tracing::warn!(error = ?e, "an export could not read the history");
        BackendError::internal(MSG_EXPORT_FAILED)
    })
}

/// The file's `is_sensitive` is a floor and never a ceiling: `copypaste_core`
/// runs the detector over every item again, so an export edited to mark a
/// credential clean comes back flagged and stays out of the search index
/// (manifest 04, PG-26).
pub(super) fn import(inner: &Inner, items: Vec<ExportItem>) -> Result<ImportData> {
    copypaste_core::import(
        &inner.state.store,
        &inner.state.detector,
        &inner.state.keyring,
        &inner.settings(),
        items,
    )
    .map_err(|e| match e {
        // Both bounds are the file answering, not a fault: neither is retryable
        // with the same file, so neither is reported as one.
        copypaste_core::ImportError::Empty => BackendError::Invalid(MSG_IMPORT_EMPTY),
        copypaste_core::ImportError::TooMany => BackendError::Invalid(MSG_IMPORT_TOO_MANY),
        e => {
            tracing::warn!(error = ?e, "an import could not be stored");
            BackendError::internal(MSG_IMPORT_FAILED)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::backend;
    use super::*;
    use crate::backend::Backend;

    /// The point of the move: on this platform these are operations, not
    /// refusals.
    #[tokio::test]
    async fn history_leaves_and_returns_without_a_daemon() {
        let (backend, _clip, _dir) = backend();
        backend.add("a shared note").await.unwrap();
        let data = backend.export(0, false).await.unwrap();
        assert_eq!(data.items.len(), 1);

        backend.clear().await.unwrap();
        assert_eq!(backend.import(data.items).await.unwrap().inserted, 1);
        assert_eq!(
            backend.list(50, None).await.unwrap().items[0].content,
            "a shared note"
        );
    }

    /// The count is what makes a short export honest, and it has to survive the
    /// hop through this backend rather than being recomputed from `items`.
    #[tokio::test]
    async fn a_withheld_item_is_counted_on_this_platform_too() {
        let (backend, _clip, _dir) = backend();
        backend.add("ordinary").await.unwrap();
        backend.add("AKIAIOSFODNN7EXAMPLE").await.unwrap();

        let default = backend.export(0, false).await.unwrap();
        assert_eq!(default.items.len(), 1);
        assert_eq!(default.skipped_sensitive, 1);
        assert_eq!(backend.export(0, true).await.unwrap().items.len(), 2);
    }

    /// PG-26 on Android. An edited export that calls a credential clean must
    /// import flagged, and must not reach the search index.
    #[tokio::test]
    async fn an_import_cannot_smuggle_a_credential_in_marked_clean() {
        let (backend, _clip, _dir) = backend();
        backend
            .import(vec![ExportItem {
                content: "AKIAIOSFODNN7EXAMPLE".into(),
                content_type: "text".into(),
                created_at: 1_700_000_000_000,
                pinned: false,
                is_sensitive: false,
            }])
            .await
            .unwrap();

        let listed = backend.list(50, None).await.unwrap();
        assert!(listed.items[0].is_sensitive, "the detector did not re-run");
        assert!(backend
            .search("AKIAIOSFODNN7EXAMPLE", 20)
            .await
            .unwrap()
            .items
            .is_empty());
    }

    /// An empty file is the file answering, not a fault — and it must not read
    /// as something this build cannot do at all.
    #[tokio::test]
    async fn an_empty_import_is_refused_as_invalid_rather_than_unsupported() {
        let (backend, _clip, _dir) = backend();
        assert!(matches!(
            backend.import(Vec::new()).await.unwrap_err(),
            BackendError::Invalid(_)
        ));
    }
}
