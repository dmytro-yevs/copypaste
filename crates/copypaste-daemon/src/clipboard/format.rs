use copypaste_ipc::content_type;

/// Return the highest-priority representation offered by a clipboard change.
/// The daemon prefers text, then the two native image formats it can store and
/// paste back without conversion.
#[cfg_attr(not(test), allow(dead_code))]
pub fn preferred<'a>(available: impl IntoIterator<Item = &'a str>) -> Option<&'static str> {
    let available: Vec<_> = available.into_iter().collect();
    [
        content_type::TEXT,
        content_type::IMAGE_PNG,
        content_type::IMAGE_TIFF,
    ]
    .into_iter()
    .find(|candidate| available.contains(candidate))
}

pub fn supports(content_type: &str) -> bool {
    matches!(
        content_type,
        content_type::TEXT | content_type::IMAGE_PNG | content_type::IMAGE_TIFF
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_and_native_images_are_capture_representations() {
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
            Some(content_type::IMAGE_PNG)
        );
        assert!(!supports(content_type::FILE));
        assert!(!supports(content_type::RICH_TEXT));
    }
}
