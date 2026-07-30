//! The value types that cross into Kotlin.
//!
//! # The one rule this file exists to enforce
//!
//! **A sensitive item's plaintext never leaves Rust as part of a list.**
//!
//! Manifest 06 INV-10 and A11Y-3 require that masked content never reaches the
//! accessibility tree, and the binding rules for this rewrite restate it:
//! "a sensitive item never renders its content, and its plaintext must not
//! reach the view tree or the accessibility tree". v1 satisfied that with CSS —
//! the plaintext was in the DOM with a blur filter over it, which is a *visual*
//! defence that TalkBack, `View.getText()`, screenshots and the Android
//! autofill/assist structure all walk straight past.
//!
//! [`ClipItem::preview`] is therefore empty for a sensitive item, and
//! [`crate::store`] does not even decrypt one to build it. There is no
//! plaintext for Compose to accidentally bind, and no accessibility node that
//! could carry it, because the string does not exist on that side of the
//! boundary. Revealing one is a separate, explicit call
//! (`CopyPaste::item_text`) that the app makes only when the user asks.

use copypaste_core::StoredItem;

/// How much of an item's text a list row is given.
///
/// The list is the only screen that gets plaintext in bulk, so it is the only
/// place worth bounding: a 32 MiB clipping should cost a row, not the process.
/// The cap is in `char`s rather than bytes so a multi-byte script is not cut
/// harder than Latin text, and the split is on a `char` boundary so the result
/// is always valid UTF-8.
///
/// Manifest 06 INV-5 ("row heights MUST be over-reserved, never estimated by
/// character count") is a *virtualiser* rule, and Compose's `LazyColumn`
/// measures rows for real rather than predicting their height, so the bug that
/// rule records cannot occur here. What survives is the reason behind it: the
/// row's line cap is a fixed constant the layout can reserve for, never a
/// function of the content.
pub const PREVIEW_CHARS: usize = 512;

/// One row of clipboard history.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ClipItem {
    /// Stable across devices and across a re-sort. Manifest 06 INV-32:
    /// selection is tracked by id, never by index — and this is that id.
    pub id: String,

    /// Text for the row, capped at [`PREVIEW_CHARS`].
    ///
    /// **Empty when [`is_sensitive`](Self::is_sensitive) is true**, always, and
    /// not as a courtesy: the plaintext is never decrypted in that case, so
    /// there is nothing here to mask, blur, or leak to an accessibility node.
    pub preview: String,

    /// `"text"`, or whatever the capturing side called it.
    pub content_type: String,

    /// Milliseconds since the Unix epoch.
    pub created_at_ms: i64,

    pub pinned: bool,

    /// A secret detector rule matched. The app must render a placeholder, and
    /// the placeholder is what TalkBack announces (A11Y-3).
    pub is_sensitive: bool,

    /// The preview was cut at [`PREVIEW_CHARS`]; the full text is longer.
    ///
    /// Always false for a sensitive item — saying "there is more" about content
    /// we refuse to show is noise.
    pub truncated: bool,
}

impl ClipItem {
    /// Build a row from a stored item plus the plaintext, if any.
    ///
    /// `plaintext` MUST be `None` for a sensitive item. The assertion is not
    /// polite about it: passing one is the bug this whole module exists to
    /// prevent, so it is dropped rather than trusted, exactly as
    /// `Store::insert` drops a `search_text` handed to it for a sensitive item.
    pub(crate) fn from_stored(item: &StoredItem, plaintext: Option<&str>) -> Self {
        let (preview, truncated) = match plaintext {
            Some(_) if item.is_sensitive => {
                tracing::warn!(
                    id = %item.id,
                    "plaintext supplied for a sensitive item's preview; dropping it"
                );
                (String::new(), false)
            }
            Some(text) => truncate(text),
            None => (String::new(), false),
        };

        Self {
            id: item.id.clone(),
            preview,
            content_type: item.content_type.clone(),
            created_at_ms: item.created_at,
            pinned: item.pinned,
            is_sensitive: item.is_sensitive,
            truncated,
        }
    }
}

/// Cut to [`PREVIEW_CHARS`] on a `char` boundary.
fn truncate(text: &str) -> (String, bool) {
    let mut out = String::new();
    for (n, c) in text.chars().enumerate() {
        if n == PREVIEW_CHARS {
            return (out, true);
        }
        out.push(c);
    }
    (out, false)
}

/// One paired device.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PairedDevice {
    /// The non-secret, stable key for this pairing — a hash of the token, not
    /// the token. Safe to log and safe to show.
    pub pairing_id: String,

    /// What the peer calls itself. Cosmetic and peer-supplied: manifest 06
    /// INV-15 requires that self-reported device details be labelled as
    /// unverified wherever they sit next to something the user must trust.
    pub name: String,

    /// `host:port` this device was last reached at, if ever.
    pub last_addr: Option<String>,

    /// Milliseconds since the Unix epoch; `0` means "never".
    pub last_seen_ms: i64,
}

/// A freshly minted pairing: the credential to read out, and the handle to
/// remember it by.
///
/// The two travel together because the screen needs both and they are needed
/// for opposite purposes. [`code`](Self::code) is shown, once, and never stored
/// or referred to again. [`pairing_id`](Self::pairing_id) is what every later
/// message, list entry and error is about. Returning only the code would force
/// each caller to re-parse it to recover the id, which sends the secret further
/// into the app than the one composable that displays it.
///
/// **[`code`](Self::code) is a live credential.** 256 bits of CSPRNG output
/// rendered as text; anyone who reads it can join the pairing. It is produced
/// exactly once and is not retrievable — nothing stores it, only the one-way
/// digest that is the id.
///
/// The app must show it and must not log it, send it anywhere, or place it on
/// the system clipboard (manifest 06 INV-14: a pairing secret is display-only,
/// so that nothing reading strings on the user's behalf can lift it).
///
/// # A caveat the Rust side cannot fix
///
/// UniFFI renders a record as a Kotlin `data class`, and a `data class` gets a
/// generated `toString()` that prints every field — including this one. The
/// [`Debug`] impl below redacts on the Rust side, but nothing here can reach
/// into the generated Kotlin. So the discipline has to live on that side too:
/// destructure this the moment it arrives, keep the id in state, and let the
/// code reach only the composable that draws it. `apps/android` does exactly
/// that in `DevicesViewModel`, and says so.
#[derive(Clone, uniffi::Record)]
pub struct NewPairing {
    /// The secret, in the human-transferable Crockford base32 form.
    pub code: String,

    /// The non-secret id derived from it — a one-way digest, not the token.
    /// Safe to log, and what every message about the pairing should use.
    pub pairing_id: String,
}

impl std::fmt::Debug for NewPairing {
    /// Redacted, and hand-written rather than derived.
    ///
    /// The module header for [`crate::pairing`] promises that nothing logs the
    /// code. A derived `Debug` would quietly make that false the first time
    /// someone wrote `tracing::debug!(?pairing, ...)` or put one in a panic
    /// message, and it would be false in exactly the places — logs, backtraces
    /// — that outlive the process and end up in bug reports.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewPairing")
            .field("pairing_id", &self.pairing_id)
            .field("code", &"<redacted>")
            .finish()
    }
}

/// What one sync attempt with one peer did.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SyncReport {
    pub pairing_id: String,
    pub name: String,
    pub sent: u32,
    pub received: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(sensitive: bool) -> StoredItem {
        StoredItem {
            id: "item-1".into(),
            content_ciphertext: vec![1, 2, 3],
            nonce: vec![4, 5, 6],
            content_type: "text".into(),
            created_at: 1_700_000_000_000,
            pinned: false,
            is_sensitive: sensitive,
        }
    }

    #[test]
    fn a_sensitive_item_carries_no_preview_even_if_plaintext_is_offered() {
        // INV-10. The caller in `store` never passes plaintext here for a
        // sensitive item; this is the layer that makes it not matter.
        let row = ClipItem::from_stored(&stored(true), Some("hunter2"));
        assert!(row.preview.is_empty());
        assert!(!row.truncated);
        assert!(row.is_sensitive);
    }

    #[test]
    fn a_sensitive_item_carries_no_preview_when_none_is_offered() {
        let row = ClipItem::from_stored(&stored(true), None);
        assert!(row.preview.is_empty());
    }

    #[test]
    fn an_ordinary_item_carries_its_text() {
        let row = ClipItem::from_stored(&stored(false), Some("hello"));
        assert_eq!(row.preview, "hello");
        assert!(!row.truncated);
    }

    #[test]
    fn a_long_preview_is_cut_and_flagged() {
        let long = "x".repeat(PREVIEW_CHARS + 10);
        let row = ClipItem::from_stored(&stored(false), Some(&long));
        assert_eq!(row.preview.chars().count(), PREVIEW_CHARS);
        assert!(row.truncated);
    }

    #[test]
    fn truncation_never_splits_a_character() {
        // Counting bytes here would slice a 4-byte emoji in half and panic, or
        // produce invalid UTF-8 across the FFI boundary.
        let emoji = "\u{1F600}".repeat(PREVIEW_CHARS + 10);
        let row = ClipItem::from_stored(&stored(false), Some(&emoji));
        assert_eq!(row.preview.chars().count(), PREVIEW_CHARS);
        assert!(row.preview.chars().all(|c| c == '\u{1F600}'));
    }

    #[test]
    fn a_preview_exactly_at_the_cap_is_not_flagged_as_truncated() {
        let exact = "y".repeat(PREVIEW_CHARS);
        let row = ClipItem::from_stored(&stored(false), Some(&exact));
        assert_eq!(row.preview, exact);
        assert!(!row.truncated);
    }
}
