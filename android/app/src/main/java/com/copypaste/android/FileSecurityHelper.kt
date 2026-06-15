package com.copypaste.android

/**
 * Filename security helpers — Android port of the denylist and sanitizer in
 * `copypaste-ui/src-tauri/src/ipc.rs` and `copypaste-core/src/filename_security.rs`.
 *
 * File items can arrive from a PAIRED PEER via P2P/relay sync.  The peer
 * controls the filename (and therefore the extension) stored by the daemon.
 * A malicious peer could send a file named `evil.sh` or `evil.apk` — when
 * the local user taps "Open", Android would fire [android.content.Intent.ACTION_VIEW]
 * and potentially execute the payload without any further prompt.
 *
 * ## Protection strategy
 * - [isDangerousExtension] — blocks direct [Intent.ACTION_VIEW] for known
 *   executable/script/bundle types.
 * - [sanitizeFilename] — strips path-traversal sequences, control characters,
 *   and shell-special characters before the file is written to the cache dir.
 *
 * The denylist is kept in sync with the Rust reference implementation.  Any
 * extension added or removed here MUST also be updated in:
 *   - `crates/copypaste-core/src/filename_security.rs` (`is_dangerous_extension`)
 *   - `crates/copypaste-ui/src-tauri/src/ipc.rs` (`is_dangerous_extension`)
 *
 * // fr44
 */
object FileSecurityHelper {

    /**
     * Return `true` when [ext] (the extension WITHOUT the leading dot, e.g.
     * `"sh"`) belongs to a known executable, script, or bundle type that must
     * NOT be opened directly via [android.content.Intent.ACTION_VIEW].
     *
     * Comparison is case-insensitive.
     */
    fun isDangerousExtension(ext: String): Boolean {
        // Explicit denylist — mirrors copypaste-core/src/filename_security.rs verbatim.
        // Err on the side of caution: add entries for new executable types;
        // never remove without a security review.
        return when (ext.lowercase()) {
            // macOS-specific execution vectors
            "app", "action", "workflow", "definition",
            "scpt", "scptd", "applescript",
            "terminal", "command", "tool",
            // Shell scripts
            "sh", "bash", "zsh", "csh", "fish", "ksh",
            // Interpreted languages
            "py", "rb", "pl", "php", "lua", "tcl", "r",
            // JavaScript (node / browser)
            "js", "mjs", "cjs",
            // JVM
            "jar", "class",
            // Windows executables / scripts (not primary target but included for safety)
            "exe", "bat", "cmd", "com", "msi", "ps1",
            "vb", "vbs", "ws", "wsf", "scr",
            // Native libraries that can be injected
            "dylib", "so", "dll",
            // Android package — most dangerous on this platform
            "apk",
            // Web / scripting vectors
            "html", "htm", "jse", "wsh",
            // Registry / shortcut (Windows)
            "reg", "lnk",
            // Package installers
            "dmg", "pkg" -> true
            else -> false
        }
    }

    /**
     * Sanitise a peer-supplied filename for safe materialisation in the cache
     * directory.
     *
     * Strips:
     *   - Path separators (`/`, `\`) — takes only the last component.
     *   - `..` components (path traversal).
     *   - Control characters (code points < 0x20 or == 0x7F).
     *   - Characters outside `[A-Za-z0-9._\- ]`.
     *   - Leading dots (prevents hidden-file creation on Unix-like systems).
     *
     * Caps the result to 255 characters. Falls back to `"clipboard_file"` when
     * the sanitised result would otherwise be empty.
     */
    fun sanitizeFilename(name: String): String {
        // Take only the last path component to defeat traversal.
        val base = name
            .replace('\\', '/')
            .split('/')
            .filter { it.isNotEmpty() && it != ".." }
            .lastOrNull()
            ?: ""

        // Keep only safe characters.
        val filtered = base.filter { c ->
            !c.isISOControl() && (c.isLetterOrDigit() || c == '.' || c == '-' || c == '_' || c == ' ')
        }

        // Strip leading dots.
        val stripped = filtered.trimStart('.')

        // Cap at 255 characters.
        val capped = if (stripped.length > 255) stripped.substring(0, 255) else stripped

        return if (capped.isEmpty()) "clipboard_file" else capped
    }
}
