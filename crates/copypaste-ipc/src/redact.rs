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

/// Replace anything path-shaped in `message` with `<path>`, preserving
/// whitespace so the rest of the sentence still reads normally.
///
/// A path may contain a space, and the one that matters does:
/// `~/Library/Application Support/CopyPaste/daemon.sock`. Redacting one token
/// at a time left `Support/CopyPaste/daemon.sock` on screen — the username was
/// gone only because it happens to sit before the space. So once a token is
/// path-shaped, following tokens carrying a `/` are absorbed into the same
/// redaction.
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
        // Absorb the continuation, then keep the whitespace that ended it so
        // the sentence still reads normally.
        let mut last = i;
        while last + 1 < tokens.len() && tokens[last + 1].trim_end().contains('/') {
            last += 1;
        }
        let token = tokens[last];
        out.push_str("<path>");
        out.push_str(&token[token.trim_end().len()..]);
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
    let core = token.trim_start_matches(['(', '[', '"', '\'', '`']);
    if core.is_empty() {
        return false;
    }
    core.starts_with('/')
        || core.starts_with("~/")
        || core.starts_with("./")
        || core.starts_with("../")
        || core.starts_with("file://")
        || core.contains("/Users/")
        || core.contains("/home/")
        // Windows-shaped, for completeness: C:\Users\name
        || (core.len() > 3 && core.as_bytes()[1] == b':' && core.contains('\\'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_unix_paths_and_keeps_the_sentence() {
        assert_eq!(
            scrub_paths("could not open /Users/alice/Library/x.sock for writing"),
            "could not open <path> for writing"
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
        assert_eq!(scrub_paths("a  /home/x  b"), "a  <path>  b");
        assert_eq!(scrub_paths("line\n/home/x\n"), "line\n<path>\n");
    }

    #[test]
    fn a_path_with_a_space_is_redacted_whole() {
        assert_eq!(
            scrub_paths("could not bind ~/Library/Application Support/CopyPaste/daemon.sock here"),
            "could not bind <path> here"
        );
        // The word after a path is kept, so the sentence survives.
        assert_eq!(
            scrub_paths("opened /home/x/db for writing"),
            "opened <path> for writing"
        );
    }

    #[test]
    fn no_username_survives_a_realistic_socket_error() {
        let leaked = "connection refused (os error 111) on \
                      /Users/dmytro/Library/Application Support/CopyPaste/daemon.sock";
        let out = scrub_paths(leaked);
        assert!(!out.contains("dmytro"), "username leaked through: {out}");
        assert!(
            !out.contains("CopyPaste"),
            "path tail leaked through: {out}"
        );
        assert!(!out.contains(".sock"), "path tail leaked through: {out}");
    }
}
