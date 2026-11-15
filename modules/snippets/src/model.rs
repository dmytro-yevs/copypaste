//! The synced records of the library and their keys.
//!
//! - `meta/<id>`: everything about a snippet except its body, plus a short
//!   preview so lists never read bodies.
//! - `body/<id>`: the text with its revision ancestry. A concealed body keeps
//!   its text in `secret_text`, which the host does not index.
//! - `collection/<id>`: a collection.
//!
//! Records merge by whole-record last-write-wins; body revisions let the
//! module recognize concurrent edits among the values sync replaced.

use serde::{Deserialize, Serialize};

pub(crate) const META: &str = "meta/";
pub(crate) const BODY: &str = "body/";
pub(crate) const COLLECTION: &str = "collection/";

/// Characters of the preview kept in a snippet's metadata.
pub(crate) const PREVIEW_CHARS: usize = 200;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Meta {
    pub(crate) title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) abbreviation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) collection_id: Option<String>,
    pub(crate) template: bool,
    pub(crate) pinned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pin_order: Option<f64>,
    pub(crate) order_key: String,
    pub(crate) concealed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) deleted_at_ms: Option<i64>,
    pub(crate) purged: bool,
    pub(crate) created_at_ms: i64,
    pub(crate) created_device: String,
    /// The start of the body; empty for a concealed snippet.
    pub(crate) preview: String,
}

impl Meta {
    /// Neither purged nor in Recently Deleted.
    pub(crate) fn live(&self) -> bool {
        !self.purged && self.deleted_at_ms.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Body {
    pub(crate) revision: String,
    pub(crate) ancestors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) secret_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rich_html: Option<String>,
}

impl Body {
    pub(crate) fn new(
        revision: String,
        ancestors: Vec<String>,
        text: String,
        rich_html: Option<String>,
        concealed: bool,
    ) -> Self {
        let (text, secret_text) = if concealed {
            (None, Some(text))
        } else {
            (Some(text), None)
        };
        Self {
            revision,
            ancestors,
            text,
            secret_text,
            rich_html,
        }
    }

    pub(crate) fn content(&self) -> &str {
        self.text
            .as_deref()
            .or(self.secret_text.as_deref())
            .unwrap_or_default()
    }

    pub(crate) fn version(&self) -> crate::conflict::BodyVersion<'_> {
        crate::conflict::BodyVersion {
            revision: &self.revision,
            ancestors: &self.ancestors,
            text: self.content(),
            rich_html: self.rich_html.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Collection {
    pub(crate) name: String,
    pub(crate) icon: String,
    pub(crate) accent: String,
    pub(crate) order_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) deleted_at_ms: Option<i64>,
}

/// A snippet or collection id usable inside a record key, kept as it is
/// when valid; otherwise a stable id derived from it, so every device maps a
/// foreign id the same way.
pub(crate) fn record_id(id: &str) -> String {
    if valid_id(id) {
        return id.to_owned();
    }
    use sha2::{Digest, Sha256};
    let digest = hex::encode(Sha256::digest(id.as_bytes()));
    format!("x-{}", &digest[..32])
}

/// Room for the longest prefix within the 128-byte key limit.
pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.')
}

pub(crate) fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(crate) fn preview(text: &str, concealed: bool) -> String {
    if concealed {
        return String::new();
    }
    text.chars().take(PREVIEW_CHARS).collect()
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}
