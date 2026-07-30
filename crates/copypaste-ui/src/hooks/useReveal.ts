/**
 * INV-11 — a revealed sensitive item re-hides two ways, independently:
 * on window blur (SCRH-7) and after 10s of being revealed (CopyPaste-5917.56).
 * An unattended screen must not stay exposed.
 *
 * Only one item can be revealed at a time; revealing another hides the first.
 */
import { useCallback, useEffect, useState } from "react";

import { REVEAL_TIMEOUT_MS } from "../lib/layout";

export function useReveal() {
  const [revealedId, setRevealedId] = useState<string | null>(null);

  const hide = useCallback(() => setRevealedId(null), []);
  const reveal = useCallback((id: string) => setRevealedId(id), []);

  useEffect(() => {
    if (revealedId === null) return;

    const timer = setTimeout(() => setRevealedId(null), REVEAL_TIMEOUT_MS);
    const onBlur = () => setRevealedId(null);

    window.addEventListener("blur", onBlur);
    return () => {
      clearTimeout(timer);
      window.removeEventListener("blur", onBlur);
    };
  }, [revealedId]);

  return { revealedId, reveal, hide };
}
