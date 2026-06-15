//! Filename security helpers shared across the daemon and other Rust crates.
//!
//! # Security context
//! File items can arrive from a PAIRED PEER via P2P/relay sync.  The peer
//! controls the filename (and therefore the extension) stored by the daemon.
//! A malicious peer could send a file named `evil.sh`, `evil.command`, or
//! `evil.app` — if the local user then opens the file, the OS might execute
//! the payload without further prompting.
//!
//! This module provides:
//!   - [`is_dangerous_extension`] — explicit denylist of executable/script/
//!     bundle extensions that should never be opened directly.
//!   - [`sanitize_filename`] — strips path-traversal sequences, shell-special
//!     characters, and control characters from a peer-supplied filename before
//!     it is stored on disk.
//!
//! The denylist mirrors `fn is_dangerous_extension` in
//! `copypaste-ui/src-tauri/src/ipc.rs` verbatim so both sides behave
//! identically.  Any extension added here MUST also be added there, and vice
//! versa — never remove an entry without a security review.
//!
//! # fr44

/// Return `true` when `ext` (the extension, **without** the leading dot, e.g.
/// `"sh"`) belongs to a known executable, script, or bundle type that should
/// **not** be opened directly with the OS default application.
///
/// The comparison is case-insensitive; `"SH"`, `"Sh"`, and `"sh"` all match.
pub fn is_dangerous_extension(ext: &str) -> bool {
    // Explicit denylist mirroring copypaste-ui/src-tauri/src/ipc.rs.
    // Err on the side of caution: add here whenever a new executable type
    // becomes relevant — never remove without security review.
    matches!(
        ext.to_ascii_lowercase().as_str(),
        // macOS-specific execution vectors
        |"app"| "action" | "workflow" | "definition"
        | "scpt" | "scptd" | "applescript"
        | "terminal" | "command" | "tool"
        // Shell scripts
        | "sh" | "bash" | "zsh" | "csh" | "fish" | "ksh"
        // Interpreted languages
        | "py" | "rb" | "pl" | "php" | "lua" | "tcl" | "r"
        // JavaScript (node / browser)
        | "js" | "mjs" | "cjs"
        // JVM
        | "jar" | "class"
        // Windows executables / scripts (not primary target but included for safety)
        | "exe" | "bat" | "cmd" | "com" | "msi" | "ps1"
        | "vb" | "vbs" | "ws" | "wsf" | "wsh" | "scr"
        // Native libraries that can be injected
        | "dylib" | "so" | "dll"
        // Android package (APK) — dangerous on Android, included here for cross-platform parity
        | "apk"
        // Web/scripting vectors
        | "html" | "htm" | "jse"
        // Registry / shortcut (Windows)
        | "reg" | "lnk"
        // Package installers
        | "dmg" | "pkg"
    )
}

/// Sanitise a peer-supplied filename for safe materialisation on disk.
///
/// Strips:
///   - Path separators (`/`, `\`) and any `..` components (path traversal).
///   - Control characters (U+0000–U+001F and U+007F).
///   - Leading dots (prevents hidden-file creation on Unix).
///   - Characters outside `[A-Za-z0-9._\- ]` (shell-special chars).
///
/// Caps the result to 255 bytes (max filename length on most filesystems).
/// Falls back to `"clipboard_file"` if the sanitised result is empty.
pub fn sanitize_filename(name: &str) -> String {
    // Take only the last path component — strip any directory prefix a peer
    // might have injected (e.g. `../../etc/passwd` → `passwd`).
    let base = name
        .replace('\\', "/")
        .split('/')
        .rfind(|c| !c.is_empty() && *c != "..")
        .unwrap_or("")
        .to_string();

    // Filter to safe characters only.
    let sanitized: String = base
        .chars()
        .filter(|c| {
            !c.is_control()
                && matches!(*c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' | ' ')
        })
        .collect();

    // Strip leading dots (prevents hidden files on Unix).
    let sanitized = sanitized.trim_start_matches('.').to_string();

    // Cap at 255 bytes.
    let sanitized = if sanitized.len() > 255 {
        // Truncate at a character boundary.
        let mut end = 255;
        while !sanitized.is_char_boundary(end) {
            end -= 1;
        }
        sanitized[..end].to_string()
    } else {
        sanitized
    };

    if sanitized.is_empty() {
        "clipboard_file".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_dangerous_extension ────────────────────────────────────────────────

    #[test]
    fn dangerous_shell_scripts() {
        for ext in &["sh", "bash", "zsh", "csh", "fish", "ksh"] {
            assert!(
                is_dangerous_extension(ext),
                "expected {ext} to be dangerous"
            );
        }
    }

    #[test]
    fn dangerous_macos_specific() {
        for ext in &[
            "app",
            "action",
            "workflow",
            "scpt",
            "scptd",
            "applescript",
            "terminal",
            "command",
            "tool",
            "definition",
        ] {
            assert!(
                is_dangerous_extension(ext),
                "expected {ext} to be dangerous"
            );
        }
    }

    #[test]
    fn dangerous_windows_executables() {
        for ext in &[
            "exe", "bat", "cmd", "com", "msi", "ps1", "vbs", "scr", "vb", "ws", "wsf",
        ] {
            assert!(
                is_dangerous_extension(ext),
                "expected {ext} to be dangerous"
            );
        }
    }

    #[test]
    fn dangerous_interpreted_languages() {
        for ext in &["py", "rb", "pl", "php", "lua", "tcl", "r"] {
            assert!(
                is_dangerous_extension(ext),
                "expected {ext} to be dangerous"
            );
        }
    }

    #[test]
    fn dangerous_javascript() {
        for ext in &["js", "mjs", "cjs"] {
            assert!(
                is_dangerous_extension(ext),
                "expected {ext} to be dangerous"
            );
        }
    }

    #[test]
    fn dangerous_jvm() {
        for ext in &["jar", "class"] {
            assert!(
                is_dangerous_extension(ext),
                "expected {ext} to be dangerous"
            );
        }
    }

    #[test]
    fn dangerous_native_libraries() {
        for ext in &["dylib", "so", "dll"] {
            assert!(
                is_dangerous_extension(ext),
                "expected {ext} to be dangerous"
            );
        }
    }

    #[test]
    fn dangerous_android_apk() {
        assert!(is_dangerous_extension("apk"));
    }

    #[test]
    fn dangerous_web_vectors() {
        for ext in &["html", "htm", "jse", "wsh", "reg", "lnk"] {
            assert!(
                is_dangerous_extension(ext),
                "expected {ext} to be dangerous"
            );
        }
    }

    #[test]
    fn dangerous_installers() {
        for ext in &["dmg", "pkg"] {
            assert!(
                is_dangerous_extension(ext),
                "expected {ext} to be dangerous"
            );
        }
    }

    #[test]
    fn case_insensitive_matching() {
        assert!(is_dangerous_extension("SH"));
        assert!(is_dangerous_extension("EXE"));
        assert!(is_dangerous_extension("Sh"));
        assert!(is_dangerous_extension("APP"));
    }

    #[test]
    fn safe_document_extensions() {
        for ext in &[
            "pdf", "txt", "png", "jpg", "docx", "xlsx", "zip", "mp4", "mp3",
        ] {
            assert!(
                !is_dangerous_extension(ext),
                "expected {ext} to be safe (not dangerous)"
            );
        }
    }

    #[test]
    fn empty_extension_is_safe() {
        assert!(!is_dangerous_extension(""));
    }

    // ── sanitize_filename ─────────────────────────────────────────────────────

    #[test]
    fn normal_filename_passes_through() {
        assert_eq!(sanitize_filename("report.pdf"), "report.pdf");
    }

    #[test]
    fn strips_unix_path_separators() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("/etc/passwd"), "passwd");
    }

    #[test]
    fn strips_windows_path_separators() {
        assert_eq!(sanitize_filename(r"C:\Windows\evil.exe"), "evil.exe");
        assert_eq!(sanitize_filename(r"..\evil.bat"), "evil.bat");
    }

    #[test]
    fn strips_control_characters() {
        // Null byte in filename
        let name = "evil\x00file.txt";
        let result = sanitize_filename(name);
        assert!(!result.contains('\x00'), "control chars must be stripped");
    }

    #[test]
    fn strips_leading_dots() {
        assert_eq!(sanitize_filename(".hidden"), "hidden");
        assert_eq!(sanitize_filename("...dotty"), "dotty");
    }

    #[test]
    fn strips_shell_special_characters() {
        // Semicolons and other shell-special chars are stripped; the slash is
        // treated as a path separator so we take the last component (.txt),
        // then strip the leading dot.
        assert_eq!(sanitize_filename("evil;rm -rf /.txt"), "txt");
        // Without a slash: the whole string is the component, and shell-special
        // chars (semicolon) are dropped.
        assert_eq!(sanitize_filename("evil;file.txt"), "evilfile.txt");
    }

    #[test]
    fn empty_input_gives_fallback() {
        assert_eq!(sanitize_filename(""), "clipboard_file");
    }

    #[test]
    fn all_stripped_gives_fallback() {
        assert_eq!(sanitize_filename("/"), "clipboard_file");
        assert_eq!(sanitize_filename("../.."), "clipboard_file");
        assert_eq!(sanitize_filename("\x01\x02\x03"), "clipboard_file");
    }

    #[test]
    fn caps_at_255_bytes() {
        let long_name: String = "a".repeat(300) + ".pdf";
        let result = sanitize_filename(&long_name);
        assert!(
            result.len() <= 255,
            "sanitized name must be at most 255 bytes, got {}",
            result.len()
        );
    }

    #[test]
    fn spaces_preserved() {
        assert_eq!(sanitize_filename("my report.pdf"), "my report.pdf");
    }

    #[test]
    fn dots_dashes_underscores_preserved() {
        assert_eq!(sanitize_filename("my-file_v2.tar.gz"), "my-file_v2.tar.gz");
    }
}
