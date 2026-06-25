/**
 * useSensitiveReveal — shared hook for sensitive-content reveal + auto-blur.
 *
 * SCRH-7 security feature: sensitive plaintext must re-hide automatically when
 * the window loses focus so an unattended machine does not leave secrets visible.
 *
 * Previously duplicated verbatim in:
 *   - src/views/HistoryView/HistoryRow.tsx (~262-263)
 *   - src/views/HistoryView/DetailsModal.tsx (~152-153)
 *
 * #17 dedup: both components now call this hook.
 *
 * The blur listener is ONLY registered when both isSensitive and maskSensitive
 * are true — attaching it for non-sensitive or unmasked items would be a
 * pointless no-op and adds unnecessary event listener churn.
 */
import { useState, useEffect } from "react";

export interface UseSensitiveRevealOptions {
  /** Whether this clipboard item is flagged as sensitive by the daemon. */
  isSensitive: boolean;
  /** Whether the global "mask sensitive items" preference is enabled. */
  maskSensitive: boolean;
}

export interface UseSensitiveRevealResult {
  /** True when the user has actively revealed the sensitive content. */
  revealed: boolean;
  /** Setter to toggle reveal state (e.g. on a "Reveal" button click). */
  setRevealed: React.Dispatch<React.SetStateAction<boolean>>;
}

/**
 * Manage per-item sensitive-content reveal state with automatic re-blur on
 * window focus loss (SCRH-7).
 *
 * @param options.isSensitive  - item's is_sensitive flag from the daemon.
 * @param options.maskSensitive - global pref: whether to mask sensitive items.
 * @returns { revealed, setRevealed }
 */
export function useSensitiveReveal({
  isSensitive,
  maskSensitive,
}: UseSensitiveRevealOptions): UseSensitiveRevealResult {
  const [revealed, setRevealed] = useState(false);

  // SCRH-7: re-hide sensitive content when the window loses focus so plaintext
  // is not left visible if the user walks away from the machine.
  // Only attach the listener when both conditions are true (avoids pointless
  // churn on non-sensitive or unmasked items).
  useEffect(() => {
    if (!isSensitive || !maskSensitive) return;
    const handleBlur = () => setRevealed(false);
    window.addEventListener("blur", handleBlur);
    return () => window.removeEventListener("blur", handleBlur);
  }, [isSensitive, maskSensitive]);

  return { revealed, setRevealed };
}
