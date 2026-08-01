//! The content-type vocabulary, and the one place it is spelled.
//!
//! `Item::content_type` crosses the socket, the peer transport, the cloud rows
//! and the database, and until now the value `"text"` was a bare literal in
//! twenty-six places across four crates — with two different opinions about
//! what counts as text already in the tree (`server::transfer::is_text` accepts
//! a `text/` prefix, `copypaste_cloud::sync::push::over_size_limit` does not).
//! That is the shape CLAUDE.md rule 1 is about, and it is the first thing that
//! would break under image, file and rich-text capture, where the question
//! "is this text?" stops having an obvious answer.
//!
//! # Capture and transport vocabulary
//!
//! The macOS capture backend currently emits plain text only; image and file
//! payloads wait for encrypted binary storage and native paste-back. Imported
//! or remote rows may still use every type below. Naming them here lets clients
//! render such a row honestly instead of showing an image as mojibake.

/// Plain text (`public.utf8-plain-text` on macOS).
pub const TEXT: &str = "text";
/// Rich text (`public.rtf` on macOS); currently accepted from imports/remotes.
pub const RICH_TEXT: &str = "text/rtf";
/// HTML (`public.html`); currently accepted from imports/remotes.
pub const HTML: &str = "text/html";
/// A PNG image (`public.png`).
pub const IMAGE_PNG: &str = "image/png";
/// A TIFF image (`public.tiff`), macOS's fallback screenshot representation.
pub const IMAGE_TIFF: &str = "image/tiff";
/// A reference to a file on disk, not the file's bytes.
pub const FILE: &str = "file";

/// Every type this build knows how to name.
///
/// A row whose type is not in here is still stored and still listed — an
/// unknown type is a peer running a newer build, not a corrupt row — but a
/// client should render it as "unsupported" rather than guess.
pub const KNOWN: &[&str] = &[TEXT, RICH_TEXT, HTML, IMAGE_PNG, IMAGE_TIFF, FILE];

/// Is this content a string the user can read, search and paste?
///
/// The `text/` prefix is accepted because rich text and HTML are string
/// payloads too, which is what every caller is actually asking.
#[must_use]
pub fn is_text(content_type: &str) -> bool {
    content_type == TEXT || content_type.starts_with("text/")
}

/// Is this content bytes that are not a string?
#[must_use]
pub fn is_binary(content_type: &str) -> bool {
    !is_text(content_type)
}

/// A short label for a type a client has no renderer for.
///
/// Fixed strings, never the raw type, and never anything derived from content:
/// manifest 01 I-9 keeps MIME types out of logs, and this is the same value
/// going somewhere a user can read it.
#[must_use]
pub fn label(content_type: &str) -> &'static str {
    match content_type {
        TEXT => "text",
        RICH_TEXT => "rich text",
        HTML => "HTML",
        IMAGE_PNG | IMAGE_TIFF => "image",
        FILE => "file",
        _ => "unsupported",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_and_its_subtypes_are_text() {
        assert!(is_text(TEXT));
        assert!(is_text(RICH_TEXT));
        assert!(is_text(HTML));
        assert!(is_text("text/plain"));
    }

    #[test]
    fn images_and_files_are_not() {
        for ct in [IMAGE_PNG, IMAGE_TIFF, FILE] {
            assert!(is_binary(ct), "{ct}");
            assert!(!is_text(ct), "{ct}");
        }
    }

    /// An unknown type must not be mistaken for text: a client that renders a
    /// future build's image bytes as a string shows the user mojibake, and an
    /// export that treats them as text writes it to a file.
    #[test]
    fn an_unknown_type_is_treated_as_binary_and_labelled_as_unsupported() {
        assert!(is_binary("application/x-not-invented-yet"));
        assert_eq!(label("application/x-not-invented-yet"), "unsupported");
    }

    #[test]
    fn every_known_type_has_a_label_that_is_not_the_fallback() {
        for ct in KNOWN {
            assert_ne!(label(ct), "unsupported", "{ct} has no label");
        }
    }
}
