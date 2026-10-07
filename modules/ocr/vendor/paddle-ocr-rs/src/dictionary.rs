
// Preserve the CTC indices and space token while accepting LF or CRLF files.
pub(crate) fn dictionary_keys(content: &str) -> Vec<String> {
    content.split('\n')
        .map(|value| value.strip_suffix('\r').unwrap_or(value).to_owned())
        .collect()
}

#[cfg(test)]
mod dictionary_tests {
    use super::dictionary_keys;

    #[test]
    fn windows_line_endings_do_not_become_recognized_characters() {
        let expected = vec!["#", "A", "\u{0457}", " ", ""];
        assert_eq!(dictionary_keys("#\r\nA\r\n\u{0457}\r\n \r\n"), expected);
        assert_eq!(dictionary_keys("#\nA\n\u{0457}\n \n"), expected);
    }
}
