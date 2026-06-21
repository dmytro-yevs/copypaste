// ---------------------------------------------------------------------------
// lib/ipc/tauriCommands.ts — Tauri-direct commands (bypass daemon IPC).
//
// These call Tauri commands on the app backend directly via invoke(), NOT
// through the Unix socket bridge that copypaste-daemon owns. They work even
// when the daemon is offline/wedged because they target the Tauri Rust side.
// ---------------------------------------------------------------------------

import { invoke } from "./transport";
import { IpcError } from "./transport";
import type { IpcReply, PairingQr, ResetDatabaseResult } from "./types";

/**
 * Play a soft system sound on copy — Maccy-style feedback.
 * Calls the `play_copy_sound` Tauri command which plays NSSound "Tink" on
 * macOS. Non-blocking and failure-safe: any error is swallowed by the Rust
 * side; this wrapper also ignores errors so a missing sound never disrupts the
 * copy flow.
 */
export async function playCopySound(): Promise<void> {
  try {
    await invoke<void>("play_copy_sound");
  } catch {
    // Sound is best-effort; never block the copy flow on a sound failure.
  }
}

/**
 * Show a rich macOS notification banner on copy via UNUserNotificationCenter.
 * Derives a human-readable title ("Text Copied" / "Image Copied" / "File
 * Copied") and a preview body (first ~160 chars of text, filename for files,
 * "Image" for images) from the item's content type and preview string, then
 * calls the `show_copy_notification` Tauri command which posts it from inside
 * the CopyPaste.app bundle so the banner shows the app icon.
 *
 * Non-blocking and failure-safe: any error is swallowed.
 *
 * @param contentType The daemon content type: "text" | "image" | "file" | "".
 * @param preview     The raw preview string from the daemon (may be empty).
 */
export async function showCopyNotification(
  contentType: string,
  preview: string
): Promise<void> {
  const { title, body } = buildNotificationContent(contentType, preview);
  try {
    await invoke<void>("show_copy_notification", { title, body });
  } catch {
    // Notification is best-effort; never block the copy flow on a notify failure.
  }
}

/** Build notification title + body from daemon content_type + preview. */
function buildNotificationContent(
  contentType: string,
  preview: string
): { title: string; body: string } {
  if (contentType === "text") {
    return { title: "Text Copied", body: truncatePreviewBody(preview) || "Copied" };
  }
  if (contentType === "image" || contentType.startsWith("image/")) {
    return { title: "Image Copied", body: "Image" };
  }
  if (contentType === "file") {
    // Daemon preview is "[file: <filename>]" — strip the wrapper.
    const inner = preview.replace(/^\[file:\s*/, "").replace(/\]$/, "").trim();
    return { title: "File Copied", body: inner || "File" };
  }
  // Fallback / unknown content type.
  return { title: "Copied", body: truncatePreviewBody(preview) || "Copied" };
}

/**
 * Truncate a raw text preview to ~160 chars at a word boundary with a
 * trailing `…`.  Preserves newlines so multi-line text reads naturally.
 */
function truncatePreviewBody(preview: string): string {
  const MAX = 160;
  const s = preview.trim();
  if (s.length <= MAX) return s;
  const cut = s.slice(0, MAX);
  const wordBoundary = Math.max(cut.lastIndexOf(" "), cut.lastIndexOf("\n"));
  const chopped = wordBoundary > 0 ? cut.slice(0, wordBoundary).trimEnd() : cut.trimEnd();
  return chopped + "…";
}

/**
 * Get the currently configured popup shortcut accelerator string
 * (e.g. "CmdOrCtrl+Shift+V").
 * This calls the Tauri command directly, NOT the daemon IPC socket.
 */
export async function getPopupShortcut(): Promise<string> {
  return invoke<string>("get_popup_shortcut");
}

/**
 * Get the built-in default popup shortcut accelerator string from the Rust
 * layer (currently "CmdOrCtrl+Shift+V").
 *
 * CopyPaste-sqw0: this is the authoritative source of the default.  Rust's
 * `DEFAULT_POPUP_SHORTCUT` constant in `src-tauri/src/lib.rs` is the single
 * source of truth.  `SettingsView.tsx` fetches this at load time via
 * `getDefaultPopupShortcut()` and uses it for the "reset to default" button,
 * so the two sides can never drift silently.
 *
 * This calls the Tauri command directly, NOT the daemon IPC socket.
 */
export async function getDefaultPopupShortcut(): Promise<string> {
  return invoke<string>("get_default_popup_shortcut");
}

/**
 * Set a new popup shortcut accelerator string at runtime and persist it.
 * Throws a plain `Error` with the error message if the accelerator is
 * invalid or already taken by another application.
 * This calls the Tauri command directly, NOT the daemon IPC socket.
 */
export async function setPopupShortcut(accelerator: string): Promise<void> {
  try {
    await invoke<void>("set_popup_shortcut", { accelerator });
  } catch (e) {
    throw new Error(String(e));
  }
}

/**
 * Generate a scannable pairing QR for this device. The Tauri backend asks the
 * daemon for a fresh pairing token and renders it as an inline SVG. Scanning it
 * from another device pairs automatically. Throws a plain `Error` on failure
 * (e.g. the daemon being offline). This calls the Tauri command directly.
 */
export async function pairingQrSvg(): Promise<PairingQr> {
  try {
    return await invoke<PairingQr>("pairing_qr_svg");
  } catch (e) {
    throw new Error(String(e));
  }
}

/**
 * Wipe and recreate the daemon's clipboard database (DESTRUCTIVE recovery).
 *
 * This is the escape hatch for a daemon stuck in degraded mode because its
 * database cannot be decrypted. It erases all local clipboard history and
 * creates a fresh empty database; the daemon recovers in-place. The Tauri
 * backend always sends `confirm = true`. Throws a plain `Error` on failure
 * (daemon offline, reset failed) so the caller can surface the real error.
 */
export async function resetDatabase(): Promise<ResetDatabaseResult> {
  let reply: IpcReply;
  try {
    reply = await invoke<IpcReply>("reset_database");
  } catch (e) {
    throw new Error(String(e));
  }
  if (!reply.ok) {
    throw new IpcError(reply.error ?? "reset_database failed", reply.error_code);
  }
  const data = (reply.data ?? {}) as Partial<ResetDatabaseResult>;
  return { reset: data.reset ?? true, ready: data.ready ?? true };
}

// ---------------------------------------------------------------------------
// Daemon UPGRADE/RESTART lifecycle (Tauri-direct — bypass daemon IPC so these
// work even when the daemon is wedged/unresponsive).
// ---------------------------------------------------------------------------

/** The app's own build version (crate version, e.g. "0.5.2"). */
export async function appVersion(): Promise<string> {
  return invoke<string>("app_version");
}

/**
 * Return the last daemon spawn error from the app-owned lifecycle, if any.
 *
 * Returns `null` when the daemon started successfully (or hasn't been
 * attempted yet). Listen for the `"daemon-spawn-result"` Tauri event for
 * real-time feedback; this command is the fallback for views that load after
 * the event fires.
 */
export async function getDaemonError(): Promise<string | null> {
  return invoke<string | null>("get_daemon_error");
}

/**
 * Restart the daemon so the freshly-installed binary takes over.
 *
 * In app-owned mode this stops the tracked child process (SIGTERM + reap) and
 * respawns the bundled binary — no launchctl involved. Throws a plain `Error`
 * with the failure message on error.
 */
export async function restartDaemon(): Promise<void> {
  try {
    await invoke<void>("restart_daemon");
  } catch (e) {
    throw new Error(String(e));
  }
}

// ---------------------------------------------------------------------------
// Accessibility permission (macOS only — always true on other platforms)
// ---------------------------------------------------------------------------

/**
 * Check whether the macOS Accessibility permission is granted for this app.
 * Returns `true` on non-macOS platforms (no permission needed there).
 * This calls the Tauri command directly, NOT the daemon IPC socket.
 */
export async function checkAccessibilityPermission(): Promise<boolean> {
  return invoke<boolean>("check_accessibility_permission");
}

/**
 * Open System Settings → Privacy & Security → Accessibility and attempt to
 * (re-)install the CGEventTap if permission was just granted.
 * No-op on non-macOS platforms.
 * This calls the Tauri command directly, NOT the daemon IPC socket.
 */
export async function requestAccessibilityPermission(): Promise<void> {
  await invoke<void>("request_accessibility_permission");
}

// ---------------------------------------------------------------------------
// Log viewer commands (Tauri-direct — bypass daemon IPC)
// ---------------------------------------------------------------------------

/**
 * Read the last `maxLines` lines from the daemon log files in
 * ~/Library/Logs/CopyPaste/. Returns the log content as a single string.
 */
export async function readLogs(maxLines: number): Promise<string> {
  return invoke<string>("read_logs", { maxLines });
}

/**
 * Return the log directory path (~/Library/Logs/CopyPaste on macOS).
 */
export async function logDirPath(): Promise<string> {
  return invoke<string>("log_dir_path");
}

/**
 * Bring the main CopyPaste window to the foreground.
 *
 * Used when an incoming pairing request arrives on the responder side so the
 * user sees the SAS confirmation modal without having to open the app manually.
 * Non-blocking and failure-safe: any error is swallowed.
 */
export async function focusMainWindow(): Promise<void> {
  try {
    await invoke<void>("focus_main_window");
  } catch {
    // Best-effort — never block the pairing flow on a window-focus failure.
  }
}

/**
 * Fire a macOS notification informing the user that a remote device is
 * requesting to pair. Reuses the existing UNUserNotificationCenter path via
 * `show_copy_notification` so no new Tauri command is needed.
 *
 * Non-blocking and failure-safe.
 *
 * @param peerName  User-visible name of the peer requesting to pair.
 */
export async function showPairingRequestNotification(peerName: string): Promise<void> {
  const title = "CopyPaste: Pairing Request";
  const body = `"${peerName}" wants to pair — open CopyPaste to confirm.`;
  try {
    await invoke<void>("show_copy_notification", { title, body });
  } catch {
    // Best-effort — notification failure must never break the pairing flow.
  }
}

/**
 * Write `text` as plain UTF-8 to the system clipboard (no rich formatting),
 * then activate the prior app and synthesise Cmd+V.
 *
 * This is the backend for the Option+Enter "paste as plain text" shortcut (F1).
 * The caller must hide the popup BEFORE calling this so the prior app receives
 * focus before the synthetic Cmd+V fires.
 *
 * On non-macOS this is a no-op.
 */
export async function pasteAsPlainText(text: string): Promise<void> {
  await invoke<void>("paste_plain_text", { text });
}

// ---------------------------------------------------------------------------
// CopyPaste-6uy9: allow-screenshots / content-protection toggle
// ---------------------------------------------------------------------------

/**
 * Return the current allow-screenshots preference.
 * `true` = screenshots allowed (content protection disabled).
 * `false` = content protection ON (default — PG-25 behaviour).
 */
export async function getAllowScreenshots(): Promise<boolean> {
  return invoke<boolean>("get_allow_screenshots");
}

/**
 * Enable or disable screenshot / screen-recording protection for all windows.
 *
 * `allow = true`  — disables NSWindowSharingNone so screen-capture tools
 *                   can capture CopyPaste windows.
 * `allow = false` — re-enables protection (PG-25 default).
 *
 * The preference is persisted to `ui-config.json` and applied immediately
 * to all open windows without a restart.
 */
export async function setAllowScreenshots(allow: boolean): Promise<void> {
  await invoke<void>("set_allow_screenshots", { allow });
}
