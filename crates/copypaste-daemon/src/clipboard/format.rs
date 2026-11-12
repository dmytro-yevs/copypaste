use copypaste_ipc::content_type;

/// Return the highest-priority representation offered by a clipboard change.
/// A native file reference wins over the textual fallback many file managers
/// publish beside it, and plain text wins over every richer form. Without
/// plain text an image wins over formatted text: browsers publish a copied
/// image beside an HTML fragment that only references it.
#[cfg_attr(not(test), allow(dead_code))]
pub fn preferred<'a>(available: impl IntoIterator<Item = &'a str>) -> Option<&'static str> {
    let available: Vec<_> = available.into_iter().collect();
    [
        content_type::FILE,
        content_type::TEXT,
        content_type::IMAGE_PNG,
        content_type::IMAGE_TIFF,
        content_type::RICH_TEXT,
        content_type::HTML,
    ]
    .into_iter()
    .find(|candidate| available.contains(candidate))
}

/// Decodes a text representation exactly as `String::from_utf8_lossy` would.
/// Valid UTF-8 keeps its buffer, so a capture never holds the read bytes and
/// a second copy of them at once.
pub fn text_from_utf8(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
}

pub fn supports(content_type: &str) -> bool {
    matches!(
        content_type,
        content_type::TEXT
            | content_type::RICH_TEXT
            | content_type::HTML
            | content_type::IMAGE_PNG
            | content_type::IMAGE_TIFF
            | content_type::FILE
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_representations_follow_the_capture_priority() {
        assert_eq!(
            preferred([
                content_type::FILE,
                content_type::IMAGE_PNG,
                content_type::TEXT
            ]),
            Some(content_type::FILE)
        );
        assert_eq!(
            preferred([
                content_type::IMAGE_PNG,
                content_type::IMAGE_TIFF,
                content_type::FILE,
                content_type::RICH_TEXT,
                content_type::HTML,
            ]),
            Some(content_type::FILE)
        );
        assert_eq!(
            preferred([
                content_type::HTML,
                content_type::RICH_TEXT,
                content_type::IMAGE_TIFF,
                content_type::IMAGE_PNG,
            ]),
            Some(content_type::IMAGE_PNG)
        );
        assert_eq!(
            preferred([
                content_type::IMAGE_PNG,
                content_type::HTML,
                content_type::TEXT
            ]),
            Some(content_type::TEXT)
        );
        assert!(supports(content_type::FILE));
        assert!(supports(content_type::RICH_TEXT));
        assert!(!supports("text/markdown"));
    }

    #[test]
    fn text_decoding_reuses_valid_bytes_and_matches_lossy_replacement() {
        let valid = "naïve — text".as_bytes().to_vec();
        let address = valid.as_ptr();
        let text = text_from_utf8(valid);
        assert_eq!(text, "naïve — text");
        assert_eq!(text.as_ptr(), address, "valid UTF-8 must not be copied");

        for malformed in [&b"a\xffb"[..], b"\xe2\x82", b"ok\xc0\xafend\xed\xa0\x80"] {
            assert_eq!(
                text_from_utf8(malformed.to_vec()),
                String::from_utf8_lossy(malformed)
            );
        }
    }
}
