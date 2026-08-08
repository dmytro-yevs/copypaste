use copypaste_ipc::content_type;

/// Return the one representation a pasteboard change is allowed to produce.
/// Keeping the product boundary platform-free makes every backend agree that
/// plain text is the only captured representation.
#[cfg_attr(not(test), allow(dead_code))]
pub fn preferred<'a>(available: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    available
        .into_iter()
        .find(|candidate| *candidate == content_type::TEXT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_the_only_capture_representation() {
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
                content_type::IMAGE_PNG,
                content_type::IMAGE_TIFF,
                content_type::FILE,
                content_type::RICH_TEXT,
                content_type::HTML,
            ]),
            None,
            "non-text-only clipboard changes are acknowledged but not captured"
        );
    }
}
