//! Linux clipboard representation policy, without display-server I/O.

pub(super) fn representation(
    available: impl Fn(&str) -> bool,
) -> Option<(&'static str, &'static str, bool)> {
    [
        ("text/uri-list", copypaste_ipc::content_type::FILE, true),
        (
            "text/plain;charset=utf-8",
            copypaste_ipc::content_type::TEXT,
            false,
        ),
        ("UTF8_STRING", copypaste_ipc::content_type::TEXT, false),
        ("text/plain", copypaste_ipc::content_type::TEXT, false),
        ("TEXT", copypaste_ipc::content_type::TEXT, false),
        ("STRING", copypaste_ipc::content_type::TEXT, false),
        ("text/rtf", copypaste_ipc::content_type::RICH_TEXT, false),
        ("text/html", copypaste_ipc::content_type::HTML, false),
        ("image/png", copypaste_ipc::content_type::IMAGE_PNG, true),
        ("image/tiff", copypaste_ipc::content_type::IMAGE_TIFF, true),
    ]
    .into_iter()
    .find(|(mime, _, _)| available(mime))
}

const WRITE_FORMATS: [(&str, &str); 9] = [
    (copypaste_ipc::content_type::HTML, "text/html"),
    (copypaste_ipc::content_type::RICH_TEXT, "text/rtf"),
    (copypaste_ipc::content_type::IMAGE_PNG, "image/png"),
    (copypaste_ipc::content_type::IMAGE_TIFF, "image/tiff"),
    (copypaste_ipc::content_type::FILE, "text/uri-list"),
    ("image/jpeg", "image/jpeg"),
    ("image/gif", "image/gif"),
    ("image/bmp", "image/bmp"),
    ("image/webp", "image/webp"),
];

pub(super) fn write_format(content_type: &str) -> Option<(&'static str, u32)> {
    WRITE_FORMATS
        .iter()
        .enumerate()
        .find_map(|(index, (kind, mime))| {
            (*kind == content_type).then_some((*mime, index as u32 + 1))
        })
}

pub(super) fn write_mime(target: u32) -> Option<&'static str> {
    WRITE_FORMATS
        .get(target.checked_sub(1)? as usize)
        .map(|(_, mime)| *mime)
}

pub(super) fn capture_limit(settings: &copypaste_ipc::ConfigData, content_type: &str) -> usize {
    settings
        .capture_limit_bytes(content_type)
        .min(copypaste_ipc::MAX_CONTENT_BYTES as u64) as usize
}

pub(super) fn local_file_url(bytes: &[u8]) -> Option<url::Url> {
    let mut uris = std::str::from_utf8(bytes)
        .ok()?
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'));
    let uri = uris.next()?;
    if uris.next().is_some() {
        return None;
    }
    let url = url::Url::parse(uri).ok()?;
    (url.scheme() == "file"
        && url
            .host_str()
            .is_none_or(|host| host.is_empty() || host.eq_ignore_ascii_case("localhost"))
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.query().is_none()
        && url.fragment().is_none())
    .then_some(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn representation_order_matches_macos_for_every_subset() {
        assert_eq!(
            representation(|mime| matches!(mime, "text/plain;charset=utf-8" | "text/html")),
            Some((
                "text/plain;charset=utf-8",
                copypaste_ipc::content_type::TEXT,
                false
            ))
        );
        let expected = [
            ("text/uri-list", copypaste_ipc::content_type::FILE, true),
            (
                "text/plain;charset=utf-8",
                copypaste_ipc::content_type::TEXT,
                false,
            ),
            ("UTF8_STRING", copypaste_ipc::content_type::TEXT, false),
            ("text/plain", copypaste_ipc::content_type::TEXT, false),
            ("TEXT", copypaste_ipc::content_type::TEXT, false),
            ("STRING", copypaste_ipc::content_type::TEXT, false),
            ("text/rtf", copypaste_ipc::content_type::RICH_TEXT, false),
            ("text/html", copypaste_ipc::content_type::HTML, false),
            ("image/png", copypaste_ipc::content_type::IMAGE_PNG, true),
            ("image/tiff", copypaste_ipc::content_type::IMAGE_TIFF, true),
        ];
        for mask in 0u32..1 << expected.len() {
            let available = |mime: &str| {
                expected
                    .iter()
                    .enumerate()
                    .any(|(index, (name, _, _))| mask & (1 << index) != 0 && mime == *name)
            };
            let selected = expected
                .iter()
                .enumerate()
                .find(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, value)| *value);
            assert_eq!(
                representation(available),
                selected,
                "offered subset {mask:#x}"
            );
        }
        assert_eq!(representation(|mime| mime == "application/x-unknown"), None);
    }

    #[test]
    fn every_macos_binary_payload_has_a_native_linux_write_format() {
        for (content_type, mime, target) in [
            (copypaste_ipc::content_type::HTML, "text/html", 1),
            (copypaste_ipc::content_type::RICH_TEXT, "text/rtf", 2),
            (copypaste_ipc::content_type::IMAGE_PNG, "image/png", 3),
            (copypaste_ipc::content_type::IMAGE_TIFF, "image/tiff", 4),
            (copypaste_ipc::content_type::FILE, "text/uri-list", 5),
            ("image/jpeg", "image/jpeg", 6),
            ("image/gif", "image/gif", 7),
            ("image/bmp", "image/bmp", 8),
            ("image/webp", "image/webp", 9),
        ] {
            assert_eq!(
                write_format(content_type),
                Some((mime, target)),
                "{content_type}"
            );
            assert_eq!(write_mime(target), Some(mime));
        }
        assert_eq!(write_format("application/x-unknown"), None);
        assert_eq!(write_mime(0), None);
        assert_eq!(write_mime(10), None);
    }

    #[test]
    fn file_body_allowance_cannot_expand_native_uri_metadata_read() {
        let settings = copypaste_ipc::ConfigData {
            max_file_size_bytes: copypaste_ipc::MAX_FILE_SIZE_BYTES,
            ..Default::default()
        };
        assert_eq!(
            capture_limit(&settings, copypaste_ipc::content_type::FILE),
            copypaste_ipc::MAX_CONTENT_BYTES
        );
        let smaller = copypaste_ipc::ConfigData {
            max_text_size_bytes: 1234,
            max_image_size_bytes: 5678,
            ..settings
        };
        assert_eq!(
            capture_limit(&smaller, copypaste_ipc::content_type::TEXT),
            1234
        );
        assert_eq!(
            capture_limit(&smaller, copypaste_ipc::content_type::IMAGE_TIFF),
            5678
        );
    }

    #[test]
    fn one_local_uri_ignores_list_comments_and_empty_lines() {
        let url = local_file_url(b"# copied files\r\n\r\nfile:///tmp/%D0%A3%D0%BA%D1%80%D0%B0%D1%97%D0%BD%D0%B0.txt\r\n\r\n").unwrap();
        assert_eq!(url.scheme(), "file");
        assert_eq!(
            url.path(),
            "/tmp/%D0%A3%D0%BA%D1%80%D0%B0%D1%97%D0%BD%D0%B0.txt"
        );
        assert!(local_file_url(b"file:///tmp/a\nfile:///tmp/b\n").is_none());
        assert!(local_file_url(b"# no files\n\n").is_none());
    }

    #[test]
    fn file_uris_follow_the_macos_local_url_gate() {
        for uri in [
            "file:///tmp/a?download=1",
            "file:///tmp/a#fragment",
            "https://example.test/a",
            "file://remote.example/tmp/a",
            "file://user:password@localhost/tmp/a",
            "file://localhost:123/tmp/a",
        ] {
            assert!(local_file_url(uri.as_bytes()).is_none(), "{uri}");
        }
        assert!(local_file_url(b"file:///tmp/local").is_some());
        assert!(local_file_url(b"file://localhost/tmp/local").is_some());
    }
}
