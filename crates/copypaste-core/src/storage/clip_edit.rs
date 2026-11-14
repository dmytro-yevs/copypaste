//! Edited and manually created clips, and snippets saved from clips.
//!
//! An edit never mutates the clip it starts from: the edited text becomes a
//! new clip through the ordinary ingest path, attributed to CopyPaste, and a
//! synced note records its provenance. Replacing the original is the normal
//! synced delete, run only after the new clip is stored. Text is stored with
//! LF line endings; the source's secret and hidden-from-integrations marks carry over.

use copypaste_ipc::content_type;
use copypaste_ipc::ConfigData;

use super::organization::{ClipNoteRecord, Provenance, ProvenanceKind, Record};
use super::snippets::{normalize_line_endings, SnippetDetail, SnippetDraft, SnippetError};
use super::{Store, StoreError, StoredItem};
use crate::ingest::{self, IngestError, Ingested};
use crate::{ClipboardPayload, CryptoError, ItemKey, Keyring, PayloadMetadata};

/// Larger clips open read-only.
pub const MAX_EDITABLE_BYTES: usize = 256 * 1024;
/// The source application of clips written in CopyPaste itself.
pub const SOURCE_APP_ID: &str = "com.copypaste.app";
pub const SOURCE_APP_NAME: &str = "CopyPaste";

#[derive(Debug, thiserror::Error)]
pub enum ClipEditError {
    #[error("the clip was not found")]
    NotFound,
    #[error("only text clips can be edited")]
    NotText,
    #[error("the clip is too large to edit")]
    TooLarge,
    #[error("the text is empty")]
    Empty,
    #[error("the rich text could not be read")]
    InvalidRichText,
    #[error("the clip could not be decrypted")]
    Decrypt(#[from] CryptoError),
    #[error("the clip could not be stored")]
    Ingest(IngestError),
    #[error(transparent)]
    Snippet(SnippetError),
    #[error("the history database could not be accessed")]
    Store(#[from] StoreError),
}

impl From<IngestError> for ClipEditError {
    fn from(error: IngestError) -> Self {
        match error {
            IngestError::Empty => Self::Empty,
            IngestError::Storage(error) => Self::Store(error),
            error => Self::Ingest(error),
        }
    }
}

impl From<SnippetError> for ClipEditError {
    fn from(error: SnippetError) -> Self {
        match error {
            SnippetError::EmptyBody => Self::Empty,
            SnippetError::Store(error) => Self::Store(error),
            error => Self::Snippet(error),
        }
    }
}

/// The outcome of saving an edit.
#[derive(Debug)]
pub struct ClipEdit {
    /// The edited clip, or the original when the text did not change.
    pub item: StoredItem,
    /// The original was tombstoned.
    pub replaced: bool,
}

/// Where new clip text comes from and how it is stored.
pub struct ClipWriter<'a, F: Fn() -> ConfigData> {
    pub store: &'a Store,
    pub keyring: &'a Keyring,
    pub settings: &'a ConfigData,
    /// The retention policy read after the write, as for every capture.
    pub current_settings: F,
}

/// A text clip opened for editing, with the marks an edited copy inherits.
struct Source {
    row: StoredItem,
    text: zeroize::Zeroizing<String>,
    secret: bool,
    hidden_from_integrations: bool,
}

fn open_text(store: &Store, key: &ItemKey, id: &str) -> Result<Source, ClipEditError> {
    let row = store.get(id)?.ok_or(ClipEditError::NotFound)?;
    if !content_type::is_text(&row.content_type) {
        return Err(ClipEditError::NotText);
    }
    let ClipboardPayload::Text(text) = ClipboardPayload::open(&row, key)? else {
        return Err(ClipEditError::NotText);
    };
    if text.len() > MAX_EDITABLE_BYTES {
        return Err(ClipEditError::TooLarge);
    }
    let annotations = store
        .item_annotations(std::slice::from_ref(&row.id))?
        .remove(&row.id);
    Ok(Source {
        secret: row.clipboard_privacy().secret
            || annotations.as_ref().is_some_and(|marks| marks.rule_secret),
        hidden_from_integrations: annotations.is_some_and(|marks| marks.hidden_from_integrations),
        text,
        row,
    })
}

fn provenance(kind: ProvenanceKind, from_clip_id: Option<String>) -> Provenance {
    Provenance {
        kind,
        from_clip_id,
        transform_snapshot: None,
        pipeline_name: None,
        module: None,
    }
}

/// The plain text of a text clip; rich clips are rendered without formatting.
fn plain(source: &Source) -> Result<String, ClipEditError> {
    crate::rich_text::plain_text(&source.row.content_type, &source.text)
        .map(|text| normalize_line_endings(&text))
        .map_err(|_| ClipEditError::InvalidRichText)
}

impl<F: Fn() -> ConfigData> ClipWriter<'_, F> {
    /// A new text clip with `manual` provenance.
    pub fn create_manual(&self, text: &str) -> Result<StoredItem, ClipEditError> {
        let text = normalize_line_endings(text);
        self.store_new(
            &text,
            false,
            false,
            provenance(ProvenanceKind::Manual, None),
        )
    }

    /// Saves `text` as an edited copy of clip `id`. Text equal to the
    /// original, raw or rendered plain, changes nothing, so a rich clip keeps
    /// its formatting and the original is never replaced by itself.
    pub fn save_edit(
        &self,
        id: &str,
        text: &str,
        replace_original: bool,
    ) -> Result<ClipEdit, ClipEditError> {
        let source = open_text(self.store, &self.keyring.item_key(), id)?;
        let text = normalize_line_endings(text);
        if text == normalize_line_endings(&source.text)
            || plain(&source).is_ok_and(|plain| plain == text)
        {
            return Ok(ClipEdit {
                item: source.row,
                replaced: false,
            });
        }
        let item = self.store_new(
            &text,
            source.secret,
            source.hidden_from_integrations,
            provenance(ProvenanceKind::Edited, Some(source.row.id.clone())),
        )?;
        let replaced =
            replace_original && item.id != source.row.id && self.store.delete(&source.row.id)?;
        Ok(ClipEdit { item, replaced })
    }

    /// Ingests `text` as a CopyPaste clip. Text already in history promotes
    /// that clip, whose own note is left as it is.
    fn store_new(
        &self,
        text: &str,
        secret: bool,
        hidden_from_integrations: bool,
        provenance: Provenance,
    ) -> Result<StoredItem, ClipEditError> {
        if text.trim().is_empty() {
            return Err(ClipEditError::Empty);
        }
        if text.len() > MAX_EDITABLE_BYTES {
            return Err(ClipEditError::TooLarge);
        }
        let mut metadata = PayloadMetadata::default();
        metadata.privacy.secret = secret;
        let ingested = ingest::ingest_into_with_capture_source_metadata_with_current_retention(
            self.store,
            self.keyring,
            text,
            content_type::TEXT,
            crate::now_ms(),
            Some(SOURCE_APP_ID),
            Some(SOURCE_APP_NAME),
            secret.then_some(&metadata),
            self.settings,
            &self.current_settings,
        )?;
        let Ingested::Stored(item) = ingested else {
            return Ok(ingested.into_item());
        };
        let now = crate::now_ms();
        let note = ClipNoteRecord {
            clip_id: item.id.clone(),
            hidden_from_integrations,
            hidden_changed_at: if hidden_from_integrations { now } else { 0 },
            base_stamp: 0,
            keep: false,
            expires_at: None,
            expiry_duration_ms: None,
            provenance: Some(provenance),
        };
        // The clip is already stored; a missing note only loses its marker.
        if let Err(error) = self
            .store
            .write_organization_record(&self.keyring.item_key(), Record::ClipNote(note))
        {
            tracing::warn!(
                ?error,
                "an edited or manual clip was stored without its note"
            );
        }
        Ok(item)
    }
}

impl Store {
    /// A new literal snippet (Template off) with the plain text of a text
    /// clip. A secret clip becomes a concealed snippet. The clip is unchanged;
    /// the link to it stays on this device.
    pub fn save_clip_as_snippet(
        &self,
        key: &ItemKey,
        clip_id: &str,
        collection_id: Option<&str>,
    ) -> Result<SnippetDetail, ClipEditError> {
        let source = open_text(self, key, clip_id)?;
        let draft = SnippetDraft {
            collection_id: collection_id.map(str::to_owned),
            body_text: plain(&source)?,
            template: false,
            concealed: source.secret,
            origin_clip_id: Some(source.row.id),
            ..Default::default()
        };
        Ok(self.save_snippet(key, &draft)?)
    }
}
