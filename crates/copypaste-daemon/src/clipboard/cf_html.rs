//! Bounded CF_HTML fragment extraction using byte offsets from its ASCII header.

pub(super) fn fragment(bytes: &[u8]) -> Option<&[u8]> {
    let mut start = None;
    let mut end = None;
    for line in bytes.split(|byte| matches!(byte, b'\r' | b'\n')) {
        if line.is_empty() {
            continue;
        }
        let line = std::str::from_utf8(line).ok()?;
        let (key, value) = line.split_once(':')?;
        match key {
            "StartFragment" => start = Some(value.trim().parse::<usize>().ok()?),
            "EndFragment" => end = Some(value.trim().parse::<usize>().ok()?),
            _ => continue,
        }
        if let (Some(start), Some(end)) = (start, end) {
            let fragment = bytes.get(start..end)?;
            std::str::from_utf8(fragment).ok()?;
            return Some(fragment);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::fragment;

    fn payload(content: &str, newline: &str) -> Vec<u8> {
        let header = format!(
            "Version:1.0{newline}StartFragment:0000000000{newline}EndFragment:0000000000{newline}"
        );
        let start = header.len();
        let header = header
            .replace(
                "StartFragment:0000000000",
                &format!("StartFragment:{start:010}"),
            )
            .replace(
                "EndFragment:0000000000",
                &format!("EndFragment:{:010}", start + content.len()),
            );
        [header.as_bytes(), content.as_bytes()].concat()
    }

    #[test]
    fn allocation_padding_does_not_invalidate_a_utf8_fragment() {
        for newline in ["\r\n", "\n", "\r"] {
            let mut bytes = payload("<b>Привіт 🦀</b>", newline);
            bytes.extend_from_slice(&[0, 0xff, 0xfe]);
            assert_eq!(fragment(&bytes), Some("<b>Привіт 🦀</b>".as_bytes()));
        }
    }

    #[test]
    fn invalid_offsets_and_incomplete_headers_are_rejected() {
        for bytes in [
            b"StartFragment:99\nEndFragment:100\n".as_slice(),
            b"StartFragment:20\nEndFragment:10\n",
            b"StartFragment:-1\nEndFragment:20\n",
            b"StartFragment:999999999999999999999999\nEndFragment:20\n",
            b"StartFragment:20\n",
        ] {
            assert_eq!(fragment(bytes), None);
        }
    }

    #[test]
    fn invalid_utf8_inside_the_fragment_is_rejected() {
        let mut bytes = payload("x", "\n");
        *bytes.last_mut().unwrap() = 0xff;
        assert_eq!(fragment(&bytes), None);
    }
}
