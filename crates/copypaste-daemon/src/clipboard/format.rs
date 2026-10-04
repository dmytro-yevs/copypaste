use copypaste_ipc::content_type;

/// Return the highest-priority representation offered by a clipboard change.
/// A native file reference wins over the textual fallback many file managers
/// publish beside it. Remaining formats retain their text-first precedence.
#[cfg_attr(not(test), allow(dead_code))]
pub fn preferred<'a>(available: impl IntoIterator<Item = &'a str>) -> Option<&'static str> {
    let available: Vec<_> = available.into_iter().collect();
    [
        content_type::FILE,
        content_type::TEXT,
        content_type::RICH_TEXT,
        content_type::HTML,
        content_type::IMAGE_PNG,
        content_type::IMAGE_TIFF,
    ]
    .into_iter()
    .find(|candidate| available.contains(candidate))
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
        assert!(supports(content_type::FILE));
        assert!(supports(content_type::RICH_TEXT));
        assert!(!supports("text/markdown"));
    }
}
