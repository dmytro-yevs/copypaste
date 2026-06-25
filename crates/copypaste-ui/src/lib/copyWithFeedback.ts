/**
 * copyWithFeedback.ts — shared copy feedback: sound + notification.
 *
 * Both Popup.tsx and HistoryView.tsx (row-click + bulk-copy paths) previously
 * duplicated the same "if playSoundOnCopy → playCopySound(); if notifyOnCopy →
 * showCopyNotification(…)" pattern. This module is the single source of truth
 * so callers can't drift (e.g. adding the notification gate but forgetting the
 * sound gate, or vice-versa).
 *
 * #16 dedup: replaces inline call sites in:
 *   - src/popup/Popup.tsx (~196-199)
 *   - src/views/HistoryView.tsx (~255-260, ~572-577)
 *
 * UX contract (must be preserved by callers):
 *   - Sound fires BEFORE notification (order matches original call sites).
 *   - Both are fire-and-forget (void) so a sound/notify failure never blocks
 *     the copy flow.
 *   - contentType / preview default to "" when the caller doesn't supply them.
 */
import { playCopySound, showCopyNotification } from "./ipc/tauriCommands";

export interface CopyFeedbackOptions {
  /** True = call playCopySound() on successful copy. */
  playSoundOnCopy: boolean;
  /** True = call showCopyNotification() on successful copy. */
  notifyOnCopy: boolean;
  /**
   * Daemon content type of the copied item (e.g. "text" | "image" | "file").
   * Passed straight through to showCopyNotification.
   * Defaults to "" when not provided (e.g. bulk-copy with heterogeneous items).
   */
  contentType?: string;
  /**
   * Raw preview string from the daemon (first ~160 chars).
   * Passed straight through to showCopyNotification.
   * Defaults to "" when not provided.
   */
  preview?: string;
}

/**
 * Fire the post-copy feedback signals (sound and/or notification) according
 * to the user's preferences. Both signals are best-effort: failures are
 * swallowed inside playCopySound / showCopyNotification and never surface to
 * the caller.
 *
 * Call this AFTER the copy IPC succeeds.
 */
export async function copyWithFeedback(opts: CopyFeedbackOptions): Promise<void> {
  const { playSoundOnCopy, notifyOnCopy, contentType = "", preview = "" } = opts;

  if (playSoundOnCopy) {
    void playCopySound();
  }
  if (notifyOnCopy) {
    void showCopyNotification(contentType, preview);
  }
}
