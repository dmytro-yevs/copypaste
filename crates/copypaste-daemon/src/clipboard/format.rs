use copypaste_ipc::content_type;

/// Return the one representation a pasteboard change is allowed to produce.
/// Keeping this order platform-free makes the macOS binding and its tests name
/// the same policy: text, then PNG/TIFF, then a file. Rich text is a fixed
/// unsupported probe in the binding manifest, not a capture representation.
#[cfg_attr(not(test), allow(dead_code))]
pub fn preferred<'a>(available: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let available: Vec<_> = available.into_iter().collect();
    [
        content_type::TEXT,
        content_type::IMAGE_PNG,
        content_type::IMAGE_TIFF,
        content_type::FILE,
    ]
    .into_iter()
    .find(|wanted| available.iter().any(|candidate| candidate == wanted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_precedence_is_text_then_image_then_file() {
        assert_eq!(
            preferred([
                content_type::FILE,
                content_type::IMAGE_PNG,
                content_type::TEXT
            ]),
            Some(content_type::TEXT)
        );
        assert_eq!(
            preferred([
                content_type::FILE,
                content_type::IMAGE_TIFF,
                content_type::IMAGE_PNG
            ]),
            Some(content_type::IMAGE_PNG)
        );
        assert_eq!(preferred([content_type::FILE]), Some(content_type::FILE));
        assert_eq!(
            preferred([content_type::RICH_TEXT, content_type::HTML]),
            None,
            "unsupported rich text must not bypass image/file precedence"
        );
    }
}
