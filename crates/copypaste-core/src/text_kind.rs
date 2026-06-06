/// Refined classification of a TEXT clip's content, for display chips.
/// This is a pure presentation hint derived from the decrypted text — it does
/// NOT change the stored content_type ("text"/"image"/"file").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextKind {
    PlainText,
    Url,
    Email,
    Phone,
    ColorHex,
    Json,
    Code,
    Number,
    FilePath,
}

impl TextKind {
    /// Stable uppercase label for UI chips.
    pub fn label(self) -> &'static str {
        match self {
            TextKind::PlainText => "TEXT",
            TextKind::Url => "URL",
            TextKind::Email => "EMAIL",
            TextKind::Phone => "PHONE",
            TextKind::ColorHex => "COLOR",
            TextKind::Json => "JSON",
            TextKind::Code => "CODE",
            TextKind::Number => "NUMBER",
            TextKind::FilePath => "PATH",
        }
    }
}

/// Classify a text payload. Order matters: more specific wins.
pub fn classify_text(s: &str) -> TextKind {
    let trimmed = s.trim();

    if trimmed.is_empty() {
        return TextKind::PlainText;
    }

    if is_url(trimmed) {
        return TextKind::Url;
    }

    if is_email(trimmed) {
        return TextKind::Email;
    }

    if is_color_hex(trimmed) {
        return TextKind::ColorHex;
    }

    if is_phone(trimmed) {
        return TextKind::Phone;
    }

    if is_number(trimmed) {
        return TextKind::Number;
    }

    if is_json(trimmed) {
        return TextKind::Json;
    }

    if is_file_path(trimmed) {
        return TextKind::FilePath;
    }

    if is_code(trimmed) {
        return TextKind::Code;
    }

    TextKind::PlainText
}

fn is_url(s: &str) -> bool {
    // No internal whitespace allowed
    if s.contains(char::is_whitespace) {
        return false;
    }
    let lower = s.to_ascii_lowercase();
    // mailto: is treated as Email, not URL — handled by caller order (url check comes first
    // but we exclude mailto here so is_email can catch it)
    if lower.starts_with("mailto:") {
        return false;
    }
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("ftp://")
}

fn is_email(s: &str) -> bool {
    // Single line, no spaces
    if s.contains(char::is_whitespace) {
        return false;
    }
    let lower = s.to_ascii_lowercase();

    // Handle mailto: prefix
    let addr = if let Some(rest) = lower.strip_prefix("mailto:") {
        rest
    } else {
        lower.as_str()
    };

    // Exactly one '@'
    let at_count = addr.chars().filter(|&c| c == '@').count();
    if at_count != 1 {
        return false;
    }

    let (local, domain) = match addr.split_once('@') {
        Some(parts) => parts,
        None => return false,
    };

    if local.is_empty() || domain.is_empty() {
        return false;
    }

    // Domain must contain a '.' and have a non-empty TLD
    let dot_pos = match domain.rfind('.') {
        Some(p) => p,
        None => return false,
    };

    let tld = &domain[dot_pos + 1..];
    if tld.is_empty() {
        return false;
    }

    // Domain part before TLD must be non-empty
    if dot_pos == 0 {
        return false;
    }

    // Local and domain must only contain reasonable email characters
    let valid_local = local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'));
    let valid_domain = domain
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));

    valid_local && valid_domain
}

fn is_color_hex(s: &str) -> bool {
    if !s.starts_with('#') {
        return false;
    }
    let hex_part = &s[1..];
    let len = hex_part.len();
    // Only allow exactly 3, 4, 6, or 8 hex digits
    if !matches!(len, 3 | 4 | 6 | 8) {
        return false;
    }
    hex_part.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_phone(s: &str) -> bool {
    // Optional leading '+', then digits/spaces/dashes/parens only
    let rest = s.strip_prefix('+').unwrap_or(s);
    if rest.is_empty() {
        return false;
    }
    // All chars must be digits, spaces, dashes, or parens
    let all_valid = rest
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, ' ' | '-' | '(' | ')'));
    if !all_valid {
        return false;
    }
    // Must have at least 7 digits
    let digit_count = s.chars().filter(|c| c.is_ascii_digit()).count();
    digit_count >= 7
}

fn is_number(s: &str) -> bool {
    // Strip optional leading sign
    let rest = s
        .strip_prefix('-')
        .or_else(|| s.strip_prefix('+'))
        .unwrap_or(s);
    if rest.is_empty() {
        return false;
    }
    // Remove thousands separators (commas) and check for at most one decimal point
    let without_sep: String = rest.chars().filter(|&c| c != ',').collect();
    if without_sep.is_empty() {
        return false;
    }
    // Count decimal points
    let dot_count = without_sep.chars().filter(|&c| c == '.').count();
    if dot_count > 1 {
        return false;
    }
    // All remaining chars must be digits or '.'
    without_sep.chars().all(|c| c.is_ascii_digit() || c == '.')
        // Must start and end with a digit (not just a dot or sign)
        && without_sep.starts_with(|c: char| c.is_ascii_digit())
        && without_sep.ends_with(|c: char| c.is_ascii_digit())
}

fn is_json(s: &str) -> bool {
    let starts_obj = s.starts_with('{') && s.ends_with('}');
    let starts_arr = s.starts_with('[') && s.ends_with(']');
    if !starts_obj && !starts_arr {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(s).is_ok()
}

fn is_file_path(s: &str) -> bool {
    // Single line only
    if s.contains('\n') || s.contains('\r') {
        return false;
    }
    if s.len() <= 1 {
        return false;
    }
    // Must start with '/', '~/', or a Windows drive letter 'C:\'
    let is_unix = s.starts_with('/') || s.starts_with("~/");
    let is_windows = s.len() >= 3
        && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && s[1..].starts_with(":\\");

    if !is_unix && !is_windows {
        return false;
    }

    // Must contain '/' or '\' (already guaranteed by prefix checks above, but be explicit)
    s.contains('/') || s.contains('\\')
}

fn is_code(s: &str) -> bool {
    let is_multiline = s.contains('\n');
    let code_signals = [
        ";",
        "{",
        "}",
        "=>",
        "fn ",
        "def ",
        "function ",
        "import ",
        "class ",
        "#include",
        "</",
    ];
    let has_signal = code_signals.iter().any(|sig| s.contains(sig));

    if is_multiline && has_signal {
        return true;
    }

    // Single-line with strong code indicators only
    if !is_multiline && (s.contains("=>") || (s.contains(';') && s.contains('{'))) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- PlainText ---

    #[test]
    fn plain_empty() {
        assert_eq!(classify_text(""), TextKind::PlainText);
    }

    #[test]
    fn plain_whitespace_only() {
        assert_eq!(classify_text("   \t\n"), TextKind::PlainText);
    }

    #[test]
    fn plain_hello_world() {
        assert_eq!(classify_text("hello world"), TextKind::PlainText);
    }

    #[test]
    fn plain_sentence() {
        assert_eq!(
            classify_text("The quick brown fox jumps over the lazy dog."),
            TextKind::PlainText
        );
    }

    // --- URL ---

    #[test]
    fn url_https() {
        assert_eq!(classify_text("https://example.com"), TextKind::Url);
    }

    #[test]
    fn url_http() {
        assert_eq!(classify_text("http://example.com/path?q=1"), TextKind::Url);
    }

    #[test]
    fn url_ftp() {
        assert_eq!(classify_text("ftp://files.example.org"), TextKind::Url);
    }

    #[test]
    fn url_uppercase_scheme() {
        assert_eq!(classify_text("HTTPS://EXAMPLE.COM"), TextKind::Url);
    }

    #[test]
    fn url_with_space_is_plain() {
        assert_eq!(
            classify_text("https://example.com/path with spaces"),
            TextKind::PlainText
        );
    }

    #[test]
    fn mailto_is_email_not_url() {
        assert_eq!(classify_text("mailto:user@example.com"), TextKind::Email);
    }

    // --- Email ---

    #[test]
    fn email_basic() {
        assert_eq!(classify_text("user@example.com"), TextKind::Email);
    }

    #[test]
    fn email_with_plus() {
        assert_eq!(classify_text("user+tag@mail.example.org"), TextKind::Email);
    }

    #[test]
    fn email_no_tld_is_plain() {
        assert_eq!(classify_text("a@b"), TextKind::PlainText);
    }

    #[test]
    fn email_no_at_is_plain() {
        assert_eq!(classify_text("userexample.com"), TextKind::PlainText);
    }

    #[test]
    fn email_two_ats_is_plain() {
        assert_eq!(classify_text("a@b@c.com"), TextKind::PlainText);
    }

    #[test]
    fn email_with_space_is_plain() {
        assert_eq!(classify_text("user @example.com"), TextKind::PlainText);
    }

    // --- ColorHex ---

    #[test]
    fn color_hex_3() {
        assert_eq!(classify_text("#fff"), TextKind::ColorHex);
    }

    #[test]
    fn color_hex_6() {
        assert_eq!(classify_text("#1a2b3c"), TextKind::ColorHex);
    }

    #[test]
    fn color_hex_8() {
        assert_eq!(classify_text("#aabbccdd"), TextKind::ColorHex);
    }

    #[test]
    fn color_hex_4() {
        assert_eq!(classify_text("#abcd"), TextKind::ColorHex);
    }

    #[test]
    fn color_hex_wrong_length_is_plain() {
        assert_eq!(classify_text("#12345"), TextKind::PlainText);
    }

    #[test]
    fn color_hex_no_hash_is_plain() {
        assert_eq!(classify_text("ffffff"), TextKind::PlainText);
    }

    #[test]
    fn color_hex_invalid_chars_is_plain() {
        assert_eq!(classify_text("#zzzzzz"), TextKind::PlainText);
    }

    // --- Phone ---

    #[test]
    fn phone_international() {
        assert_eq!(classify_text("+1 (800) 555-1234"), TextKind::Phone);
    }

    #[test]
    fn phone_digits_only() {
        assert_eq!(classify_text("1234567890"), TextKind::Phone);
    }

    #[test]
    fn phone_too_short_is_plain() {
        // 6 digits with phone-like formatting — below 7-digit threshold, not a plain number
        assert_eq!(classify_text("12-3456"), TextKind::PlainText);
    }

    #[test]
    fn phone_with_alpha_is_not_phone() {
        assert_eq!(classify_text("+1abc5551234"), TextKind::PlainText);
    }

    // --- Number ---

    #[test]
    fn number_integer() {
        assert_eq!(classify_text("42"), TextKind::Number);
    }

    #[test]
    fn number_decimal() {
        assert_eq!(classify_text("3.14"), TextKind::Number);
    }

    #[test]
    fn number_negative() {
        assert_eq!(classify_text("-7.5"), TextKind::Number);
    }

    #[test]
    fn number_thousands_sep() {
        assert_eq!(classify_text("1,234.56"), TextKind::Number);
    }

    #[test]
    fn number_large_int_with_commas() {
        assert_eq!(classify_text("1,000,000"), TextKind::Number);
    }

    #[test]
    fn number_with_alpha_is_plain() {
        assert_eq!(classify_text("42px"), TextKind::PlainText);
    }

    #[test]
    fn number_just_dot_is_plain() {
        assert_eq!(classify_text("."), TextKind::PlainText);
    }

    // --- JSON ---

    #[test]
    fn json_object() {
        assert_eq!(classify_text(r#"{"key": "value"}"#), TextKind::Json);
    }

    #[test]
    fn json_array() {
        assert_eq!(classify_text("[1, 2, 3]"), TextKind::Json);
    }

    #[test]
    fn json_nested() {
        assert_eq!(classify_text(r#"{"a": {"b": [1, 2]}}"#), TextKind::Json);
    }

    #[test]
    fn json_invalid_braces_is_plain() {
        // Starts with '{' but is not valid JSON
        assert_eq!(classify_text("{not json}"), TextKind::PlainText);
    }

    #[test]
    fn json_empty_object() {
        assert_eq!(classify_text("{}"), TextKind::Json);
    }

    #[test]
    fn json_empty_array() {
        assert_eq!(classify_text("[]"), TextKind::Json);
    }

    // --- FilePath ---

    #[test]
    fn filepath_absolute_unix() {
        assert_eq!(classify_text("/usr/local/bin/cargo"), TextKind::FilePath);
    }

    #[test]
    fn filepath_home_relative() {
        assert_eq!(classify_text("~/Documents/notes.txt"), TextKind::FilePath);
    }

    #[test]
    fn filepath_windows() {
        assert_eq!(
            classify_text("C:\\Users\\Alice\\file.txt"),
            TextKind::FilePath
        );
    }

    #[test]
    fn filepath_no_prefix_is_plain() {
        assert_eq!(classify_text("relative/path/to/file"), TextKind::PlainText);
    }

    #[test]
    fn filepath_single_slash_is_root() {
        // "/" alone is length 1, excluded by len > 1 rule
        assert_eq!(classify_text("/"), TextKind::PlainText);
    }

    #[test]
    fn filepath_multiline_is_not_path() {
        assert_eq!(classify_text("/usr/bin\n/etc/passwd"), TextKind::PlainText);
    }

    // --- Code ---

    #[test]
    fn code_rust_multiline() {
        let src = "fn main() {\n    println!(\"hello\");\n}";
        assert_eq!(classify_text(src), TextKind::Code);
    }

    #[test]
    fn code_python_multiline() {
        let src = "def foo(x):\n    return x + 1";
        assert_eq!(classify_text(src), TextKind::Code);
    }

    #[test]
    fn code_js_arrow_single_line() {
        assert_eq!(classify_text("const f = x => x * 2"), TextKind::Code);
    }

    #[test]
    fn code_import_multiline() {
        let src = "import React from 'react';\nimport { useState } from 'react';";
        assert_eq!(classify_text(src), TextKind::Code);
    }

    #[test]
    fn code_html_tag() {
        let src = "<div>\n  <p>hello</p>\n</div>";
        assert_eq!(classify_text(src), TextKind::Code);
    }

    #[test]
    fn code_c_include_multiline() {
        let src = "#include <stdio.h>\nint main() { return 0; }";
        assert_eq!(classify_text(src), TextKind::Code);
    }

    // --- Label ---

    #[test]
    fn labels_correct() {
        assert_eq!(TextKind::PlainText.label(), "TEXT");
        assert_eq!(TextKind::Url.label(), "URL");
        assert_eq!(TextKind::Email.label(), "EMAIL");
        assert_eq!(TextKind::Phone.label(), "PHONE");
        assert_eq!(TextKind::ColorHex.label(), "COLOR");
        assert_eq!(TextKind::Json.label(), "JSON");
        assert_eq!(TextKind::Code.label(), "CODE");
        assert_eq!(TextKind::Number.label(), "NUMBER");
        assert_eq!(TextKind::FilePath.label(), "PATH");
    }

    // --- Tricky edge cases ---

    #[test]
    fn trimmed_url_with_surrounding_spaces() {
        assert_eq!(classify_text("  https://example.com  "), TextKind::Url);
    }

    #[test]
    fn number_positive_sign() {
        assert_eq!(classify_text("+42"), TextKind::Number);
    }

    #[test]
    fn json_string_only_no_braces_is_plain() {
        assert_eq!(classify_text("\"just a string\""), TextKind::PlainText);
    }

    #[test]
    fn color_hex_uppercase() {
        assert_eq!(classify_text("#AABBCC"), TextKind::ColorHex);
    }

    #[test]
    fn phone_with_dashes() {
        assert_eq!(classify_text("555-867-5309"), TextKind::Phone);
    }

    #[test]
    fn filepath_deep_path() {
        assert_eq!(
            classify_text("/home/user/.config/copypaste/settings.json"),
            TextKind::FilePath
        );
    }
}
