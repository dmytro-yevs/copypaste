//! Deterministic semantic classification for complete text clips.

use copypaste_ipc::SemanticKind;

/// Presentation metadata derived from authenticated text content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticClassification {
    pub kind: SemanticKind,
    /// Packed as `0xRRGGBBAA` for a color and absent for every other kind.
    pub color_rgba: Option<u32>,
}

/// Classify one complete text clip. Binary content has no semantic kind.
///
/// Recognition is intentionally whole-value and conservative: prose that
/// merely contains a link, address, number, or code fragment stays plain text.
#[must_use]
pub fn classify_semantic(content_type: &str, content: &str) -> Option<SemanticClassification> {
    if !copypaste_ipc::content_type::is_text(content_type) {
        return None;
    }

    let value = content.trim();
    if !is_plain_text_representation(content_type) || value.is_empty() {
        return Some(classification(SemanticKind::PlainText));
    }

    if let Some(color_rgba) = color_rgba(value) {
        return Some(SemanticClassification {
            kind: SemanticKind::Color,
            color_rgba: Some(color_rgba),
        });
    }
    if is_email(value) {
        return Some(classification(SemanticKind::Email));
    }
    if is_link(value) {
        return Some(classification(SemanticKind::Link));
    }
    if is_phone(value) {
        return Some(classification(SemanticKind::Phone));
    }
    if is_json(value) {
        return Some(classification(SemanticKind::Json));
    }
    if is_path(value) {
        return Some(classification(SemanticKind::Path));
    }
    if is_code(value) {
        return Some(classification(SemanticKind::Code));
    }
    Some(classification(SemanticKind::PlainText))
}

const fn classification(kind: SemanticKind) -> SemanticClassification {
    SemanticClassification {
        kind,
        color_rgba: None,
    }
}

fn is_plain_text_representation(content_type: &str) -> bool {
    content_type == copypaste_ipc::content_type::TEXT
        || content_type.eq_ignore_ascii_case("text/plain")
        || content_type
            .get(..11)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("text/plain;"))
}

fn color_rgba(value: &str) -> Option<u32> {
    let lower = value.to_ascii_lowercase();
    let accepted_syntax = if let Some(hex) = lower.strip_prefix('#') {
        matches!(hex.len(), 3 | 4 | 6 | 8) && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
    } else {
        [
            "rgb(", "rgba(", "hsl(", "hsla(", "hwb(", "lab(", "lch(", "oklab(", "oklch(",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
            && lower.ends_with(')')
    };
    if !accepted_syntax {
        return None;
    }

    let [red, green, blue, alpha] = csscolorparser::parse(value).ok()?.to_rgba8();
    Some(u32::from_be_bytes([red, green, blue, alpha]))
}

fn is_email(value: &str) -> bool {
    let parsed_mailto;
    let address = if value
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("mailto:"))
    {
        let Ok(url) = url::Url::parse(value) else {
            return false;
        };
        if url.scheme() != "mailto" || url.path().contains(',') {
            return false;
        }
        parsed_mailto = url;
        parsed_mailto.path()
    } else {
        value
    };

    if address.len() > 254 || !address.is_ascii() || address.chars().any(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = address.split_once('@') else {
        return false;
    };
    if local.is_empty()
        || local.len() > 64
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
        || domain.len() > 253
        || !domain.contains('.')
    {
        return false;
    }
    if !local.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'.' | b'!'
                    | b'#'
                    | b'$'
                    | b'%'
                    | b'&'
                    | b'\''
                    | b'*'
                    | b'+'
                    | b'-'
                    | b'/'
                    | b'='
                    | b'?'
                    | b'^'
                    | b'_'
                    | b'`'
                    | b'{'
                    | b'|'
                    | b'}'
                    | b'~'
            )
    }) {
        return false;
    }

    domain.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && label
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            && label
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric)
    })
}

fn is_link(value: &str) -> bool {
    if value.chars().any(char::is_whitespace) {
        return false;
    }
    let candidate = if value
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("www."))
    {
        format!("https://{value}")
    } else {
        value.to_owned()
    };
    let Ok(url) = url::Url::parse(&candidate) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https") && url.host_str().is_some()
}

fn is_phone(value: &str) -> bool {
    if value.len() > 40
        || !value
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '+' | '-' | '.' | '(' | ')' | ' '))
        || value.matches('+').count() > 1
        || (value.contains('+') && !value.starts_with('+'))
        || value.matches('(').count() != value.matches(')').count()
    {
        return false;
    }
    let digits = value.bytes().filter(u8::is_ascii_digit).count();
    if !(7..=15).contains(&digits) {
        return false;
    }
    if value.parse::<std::net::IpAddr>().is_ok() {
        return false;
    }

    let groups = value
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|group| !group.is_empty())
        .map(str::len)
        .collect::<Vec<_>>();
    if groups.as_slice() == [4, 2, 2] {
        return false;
    }

    value.starts_with('+') || value.contains(['(', ')']) || value.contains([' ', '-', '.'])
}

fn is_json(value: &str) -> bool {
    matches!(
        serde_json::from_str::<serde_json::Value>(value),
        Ok(serde_json::Value::Object(_) | serde_json::Value::Array(_))
    )
}

fn is_path(value: &str) -> bool {
    if value.contains(['\n', '\r', '\0']) {
        return false;
    }
    if value
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file://"))
    {
        return url::Url::parse(value).is_ok_and(|url| url.scheme() == "file");
    }
    if value.starts_with("~/") || value.starts_with("/Volumes/") || value.starts_with("/Users/") {
        return value.len() > 2;
    }
    if let Some(path) = value.strip_prefix('/') {
        return value.len() > 1 && (path.contains('/') || !path.contains(char::is_whitespace));
    }
    if let Some(path) = value.strip_prefix("\\\\") {
        return path.contains('\\');
    }
    let bytes = value.as_bytes();
    bytes.len() > 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
}

fn is_code(value: &str) -> bool {
    if value.starts_with("```")
        || value.starts_with("#!")
        || value.starts_with("<?xml")
        || value.starts_with("<!DOCTYPE")
    {
        return true;
    }
    if value.starts_with('<') && value.ends_with('>') && value.contains("</") {
        return true;
    }

    let lower = value.to_ascii_lowercase();
    if [
        "git ",
        "npm ",
        "pnpm ",
        "yarn ",
        "cargo ",
        "docker ",
        "kubectl ",
        "flutter ",
        "dart ",
        "python ",
        "pip ",
        "brew ",
        "sudo ",
        "print(",
        "console.log(",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
    {
        return true;
    }

    let mut score = 0u8;
    for token in ["=>", "::", "();", "</", "/>", ":=", "&&", "||", "#!/"] {
        if value.contains(token) {
            score = score.saturating_add(2);
        }
    }
    for token in ['{', '}', ';'] {
        if value.contains(token) {
            score = score.saturating_add(1);
        }
    }
    if value.contains('=') && !value.contains(" = ") {
        score = score.saturating_add(1);
    }
    if value.lines().any(|line| {
        let trimmed = line.trim_start();
        line.len() != trimmed.len() || trimmed.starts_with("//") || trimmed.starts_with("/*")
    }) {
        score = score.saturating_add(1);
    }

    let words = lower
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    for keyword in [
        "fn", "class", "struct", "enum", "impl", "def", "function", "const", "var", "return",
        "import", "export", "package", "use", "pub", "async", "await", "select", "from", "where",
        "insert", "update", "delete", "create", "table",
    ] {
        if words.contains(&keyword) {
            score = score.saturating_add(1);
        }
    }
    if [
        "let ", "const ", "var ", "fn ", "def ", "class ", "struct ", "enum ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
    {
        score = score.saturating_add(2);
    }

    let threshold = if value.contains('\n') { 2 } else { 3 };
    score >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(value: &str) -> SemanticKind {
        classify_semantic(copypaste_ipc::content_type::TEXT, value)
            .unwrap()
            .kind
    }

    #[test]
    fn recognizes_the_supported_semantic_kinds() {
        for value in ["https://example.com/path?q=1", "www.example.com/path"] {
            assert_eq!(kind(value), SemanticKind::Link, "{value}");
        }
        for value in ["person@example.com", "mailto:person@example.com"] {
            assert_eq!(kind(value), SemanticKind::Email, "{value}");
        }
        for value in ["+1 (202) 555-0101", "044 123 45 67"] {
            assert_eq!(kind(value), SemanticKind::Phone, "{value}");
        }
        for value in ["/Users/example/report.txt", r"C:\Users\example\report.txt"] {
            assert_eq!(kind(value), SemanticKind::Path, "{value}");
        }
        for value in [
            "fn main() { println!(\"hello\"); }",
            "SELECT name FROM people WHERE active = 1",
            "git status",
        ] {
            assert_eq!(kind(value), SemanticKind::Code, "{value}");
        }
        assert_eq!(kind(r#"{"name":"CopyPaste"}"#), SemanticKind::Json);
    }

    #[test]
    fn recognizes_every_agreed_color_syntax_and_provides_a_swatch() {
        for value in [
            "#abc",
            "#abcd",
            "#aabbcc",
            "#aabbccdd",
            "rgb(12 34 56 / 50%)",
            "rgba(12, 34, 56, 0.5)",
            "hsl(120 50% 50%)",
            "hsla(120, 50%, 50%, 0.5)",
            "hwb(120 20% 30%)",
            "lab(50% 20 30)",
            "lch(50% 20 30)",
            "oklab(50% 0.1 0.1)",
            "oklch(50% 0.1 30)",
        ] {
            let classification =
                classify_semantic(copypaste_ipc::content_type::TEXT, value).unwrap();
            assert_eq!(classification.kind, SemanticKind::Color, "{value}");
            assert!(classification.color_rgba.is_some(), "{value}");
        }
    }

    #[test]
    fn keeps_embedded_and_ambiguous_values_as_plain_text() {
        for value in [
            "See https://example.com for details",
            "Email person@example.com tomorrow",
            "red",
            "2026-10-03",
            "192.168.1.1",
            "1700000000",
            "let me know",
            "123",
            "true",
            r#""a JSON string""#,
        ] {
            assert_eq!(kind(value), SemanticKind::PlainText, "{value}");
        }
    }

    #[test]
    fn rich_text_and_binary_payloads_do_not_get_misclassified() {
        assert_eq!(
            classify_semantic(
                copypaste_ipc::content_type::HTML,
                "<a href=\"https://example.com\">"
            ),
            Some(classification(SemanticKind::PlainText))
        );
        assert_eq!(
            classify_semantic(copypaste_ipc::content_type::IMAGE_PNG, "#ffffff"),
            None
        );
    }
}
