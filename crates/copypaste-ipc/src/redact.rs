//! Keeping filesystem paths out of text shown to a user.
//!
//! The daemon socket lives under the user's home directory, so its path spells
//! out the local username. Any error text that reaches a screen — or a
//! screenshot in a bug report — must not carry one.
//!
//! This lives beside the wire types because every client needs it and none of
//! them can import it from another: the CLI is a binary with no lib target, and
//! the Tauri bridge is a separate crate. It was written twice before landing
//! here, which is the pattern this rewrite exists to stop. A third client must
//! call this rather than copy it.
//!
//! The daemon is expected not to send paths in the first place. This is the
//! second layer: a client-side net so a daemon-side regression degrades to an
//! unhelpful message instead of a disclosure.

/// Replace anything path-shaped in `message` with `<path>`.
///
/// Whitespace cannot safely end an unquoted path: it may be a filename tail.
/// Redact its current line rather than guess. An unescaped closing quote is an
/// explicit boundary, so text after a quoted path remains useful. Line endings
/// stay intact and URLs or ordinary slash prose never enter the path branch.
pub fn scrub_paths(message: &str) -> String {
    let tokens: Vec<&str> = message.split_inclusive(char::is_whitespace).collect();
    let mut out = String::with_capacity(message.len());
    let mut i = 0;
    while i < tokens.len() {
        let trimmed = tokens[i].trim_end();
        if !looks_like_path(trimmed) {
            out.push_str(tokens[i]);
            i += 1;
            continue;
        }
        let mut quote = opening_quote(trimmed);
        let mut last = i;
        while last + 1 < tokens.len()
            && match quote {
                Some(delimiter) if ends_with_quote(tokens[last], delimiter) => false,
                Some(delimiter) if ends_with_escaped_quote(tokens[last], delimiter) => {
                    quote = None;
                    true
                }
                Some(_) => true,
                None => !ends_line(tokens[last]),
            }
        {
            last += 1;
        }
        let token = tokens[last];
        out.push_str("<path>");
        if quote.is_some() || ends_line(token) {
            out.push_str(&token[token.trim_end().len()..]);
        }
        i = last + 1;
    }
    out
}

/// Whether a whitespace-delimited token looks like a filesystem path.
///
/// Deliberately eager: a false positive costs one unhelpful word in an error
/// message, a false negative leaks the username.
fn looks_like_path(token: &str) -> bool {
    // A message may wrap a path in punctuation — strip it before deciding.
    let core = token.trim_start_matches(['(', '[', '{', '"', '\'', '`']);
    if core.is_empty() {
        return false;
    }
    if starts_with_ignore_ascii_case(core, "http://")
        || starts_with_ignore_ascii_case(core, "https://")
    {
        return false;
    }
    if starts_with_ignore_ascii_case(core, "file://") {
        return true;
    }
    let core = path_value(core).trim_start_matches(['"', '\'', '`']);
    if starts_with_ignore_ascii_case(core, "http://")
        || starts_with_ignore_ascii_case(core, "https://")
    {
        return false;
    }

    core.starts_with('/')
        || core.starts_with("~/")
        || core.starts_with("~\\")
        || core.starts_with("./")
        || core.starts_with(".\\")
        || core.starts_with("../")
        || core.starts_with("..\\")
        || starts_with_ignore_ascii_case(core, "file://")
        || core.contains("/Users/")
        || core.contains("/home/")
        || starts_with_path_variable(core)
        || core.starts_with("\\\\")
        || starts_with_windows_drive(core)
        || core.starts_with("copypaste-v2.db")
}

fn path_value(token: &str) -> &str {
    token
        .split_once('=')
        .filter(|(label, _)| is_label(label))
        .or_else(|| token.split_once(':').filter(|(label, _)| is_label(label)))
        .map_or(token, |(_, value)| value)
}

fn is_label(value: &str) -> bool {
    value.len() > 1
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}

fn starts_with_path_variable(value: &str) -> bool {
    if let Some(rest) = value.strip_prefix('$') {
        let variable_end = rest
            .bytes()
            .position(|byte| !(byte.is_ascii_alphanumeric() || byte == b'_'))
            .unwrap_or(rest.len());
        return variable_end > 0 && matches!(rest.as_bytes().get(variable_end), Some(b'/' | b'\\'));
    }

    value
        .strip_prefix('%')
        .and_then(|rest| rest.find('%').map(|end| (end, rest)))
        .is_some_and(|(end, rest)| {
            end > 0 && matches!(rest.as_bytes().get(end + 1), Some(b'/' | b'\\'))
        })
}

fn starts_with_windows_drive(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() > 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

fn opening_quote(token: &str) -> Option<char> {
    path_value(token.trim_start_matches(['(', '[', '{']))
        .chars()
        .next()
        .filter(|character| matches!(character, '"' | '\'' | '`'))
}

fn ends_with_quote(token: &str, quote: char) -> bool {
    token
        .trim_end()
        .trim_end_matches([')', ']', '}', ',', ';', '.'])
        .strip_suffix(quote)
        .is_some_and(|before_quote| !before_quote.ends_with('\\'))
}

fn ends_with_escaped_quote(token: &str, quote: char) -> bool {
    token
        .trim_end()
        .trim_end_matches([')', ']', '}', ',', ';', '.'])
        .strip_suffix(quote)
        .is_some_and(|before_quote| before_quote.ends_with('\\'))
}

fn ends_line(token: &str) -> bool {
    token.ends_with('\r') || token.ends_with('\n')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Corpus {
        display_leak_cases: Vec<DisplayCase>,
        safe_display_cases: Vec<SafeCase>,
        #[serde(default)]
        redaction_cases: Vec<RedactionCase>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct DisplayCase {
        id: String,
        surface: String,
        expected_surface: String,
        forbidden_fragments: Vec<String>,
    }

    #[derive(Deserialize)]
    struct SafeCase {
        id: String,
        surface: String,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RedactionCase {
        id: String,
        surface: String,
        expected_surface: String,
        forbidden_fragments: Vec<String>,
    }

    fn corpus() -> Corpus {
        serde_json::from_str(include_str!(
            "../../../test-support/security/path-security-vectors.json"
        ))
        .expect("path security corpus is valid")
    }

    #[test]
    fn shared_redaction_path_security_corpus() {
        let corpus = corpus();
        for case in corpus.display_leak_cases {
            let rendered = scrub_paths(&case.surface);
            assert_eq!(
                rendered, case.expected_surface,
                "{} rendered unexpectedly",
                case.id
            );
            for fragment in case.forbidden_fragments {
                assert!(
                    !rendered.contains(&fragment),
                    "{} kept a forbidden fragment",
                    case.id
                );
            }
        }
        for case in corpus.safe_display_cases {
            assert_eq!(
                scrub_paths(&case.surface),
                case.surface,
                "{} was changed",
                case.id
            );
        }
        for case in corpus.redaction_cases {
            let rendered = scrub_paths(&case.surface);
            assert_eq!(
                rendered, case.expected_surface,
                "{} rendered unexpectedly",
                case.id
            );
            for fragment in case.forbidden_fragments {
                assert!(
                    !rendered.contains(&fragment),
                    "{} kept a forbidden fragment",
                    case.id
                );
            }
        }
    }

    #[test]
    fn redacts_unquoted_paths_to_the_line_boundary() {
        assert_eq!(
            scrub_paths("could not open /Users/alice/Library/x.sock for writing"),
            "could not open <path>"
        );
        assert_eq!(
            scrub_paths("failed at /home/bob/.local/share/db"),
            "failed at <path>"
        );
    }

    #[test]
    fn redacts_relative_home_and_url_forms() {
        for input in [
            "~/Library/Application Support/x",
            "./data/db",
            "../secrets",
            "file:///home/carol/x",
        ] {
            let out = scrub_paths(input);
            assert!(
                out.starts_with("<path>"),
                "{input} should have been redacted, got {out}"
            );
        }
    }

    #[test]
    fn redacts_paths_wrapped_in_punctuation() {
        assert_eq!(scrub_paths("(/home/dan/x)"), "<path>");
        assert_eq!(scrub_paths("\"/Users/eve/y\""), "<path>");
    }

    #[test]
    fn leaves_ordinary_text_alone() {
        let msg = "the daemon is not running; start it and try again";
        assert_eq!(scrub_paths(msg), msg);
        // A bare word with a slash inside is not a path.
        assert_eq!(scrub_paths("read/write error"), "read/write error");
    }

    #[test]
    fn preserves_whitespace_shape() {
        assert_eq!(scrub_paths("  /home/x  b"), "  <path>");
        assert_eq!(scrub_paths("  /home/x  "), "  <path>");
        assert_eq!(scrub_paths("line\n/home/x\n"), "line\n<path>\n");
    }

    #[test]
    fn a_path_with_a_space_is_redacted_whole() {
        assert_eq!(
            scrub_paths("could not bind ~/Library/Application Support/com.copypaste.CopyPaste/daemon.sock here"),
            "could not bind <path>"
        );
        assert_eq!(
            scrub_paths("opened /home/x/db for writing"),
            "opened <path>"
        );
    }

    #[test]
    fn unquoted_path_tails_do_not_cross_line_boundaries() {
        assert_eq!(
            scrub_paths(
                "could not open /Users/alice/Library Application Support/private.sock while syncing\nretry later"
            ),
            "could not open <path>\nretry later"
        );
        assert_eq!(
            scrub_paths(
                "could not open /Users/alice/Library Application Support/private.sock while syncing\r\nretry later"
            ),
            "could not open <path>\r\nretry later"
        );
    }

    #[test]
    fn no_username_survives_a_realistic_socket_error() {
        let leaked = "connection refused (os error 111) on \
                      /Users/dmytro/Library/Application Support/com.copypaste.CopyPaste/daemon.sock";
        let out = scrub_paths(leaked);
        assert!(!out.contains("dmytro"), "username leaked through: {out}");
        assert!(
            !out.contains("CopyPaste"),
            "path tail leaked through: {out}"
        );
        assert!(!out.contains(".sock"), "path tail leaked through: {out}");
    }
}
