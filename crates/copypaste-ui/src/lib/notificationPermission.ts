/**
 * notificationPermission.ts — CopyPaste-44rq.28
 *
 * Provides a testable abstraction for querying the macOS
 * UNUserNotificationCenter permission state from the Tauri frontend.
 *
 * ## Why not Notification.permission?
 * In a Tauri WKWebView the browser Notification API is a separate permission
 * system from macOS UNUserNotificationCenter. `Notification.permission` will
 * frequently be "default" even when macOS system notifications are denied (and
 * vice versa), so it is meaningless as a signal for whether
 * `show_copy_notification` will actually deliver a banner.
 *
 * ## Correct approach
 * Query the Tauri `check_notification_permission` command which delegates to
 * `UNUserNotificationCenter.current().notificationSettings.authorizationStatus`
 * on the macOS side and returns `true` when the status is `.authorized`.
 *
 * ## Dependency
 * Requires the `check_notification_permission` Tauri command to be registered
 * in `src-tauri/src/lib.rs`. Until that command exists, callers receive
 * `true` (optimistic: assume granted, show no warning) so the UI is not broken
 * by missing infrastructure.
 */

import { invoke } from "@tauri-apps/api/core";

/**
 * Returns `true` when macOS system notification permission is granted for
 * CopyPaste, `false` when it is denied or the request timed out.
 *
 * Resolves optimistically to `true` on any error so callers don't display a
 * spurious "Notifications disabled" warning while the permission check is
 * unavailable (e.g., before the Tauri command is wired up on the Rust side).
 *
 * @returns Promise<boolean>  true = permission granted (or unknown), false = denied
 */
export async function isNotificationPermissionGranted(): Promise<boolean> {
  try {
    return await invoke<boolean>("check_notification_permission");
  } catch {
    // The Tauri command is not yet registered (Rust side TODO) or we are
    // running in a test environment without a real Tauri backend.
    // Fall back to "granted" so no spurious warning is shown.
    return true;
  }
}
