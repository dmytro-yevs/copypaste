/**
 * Revealing one sensitive item, and un-revealing it again.
 *
 * A sensitive item arrives with `content: null` — the bridge drops the
 * plaintext before serialising (INV-10), so revealing is a *fetch*, not a CSS
 * state. That makes the two expiry rules literal rather than cosmetic:
 *
 *   INV-11 — a revealed item re-hides on **window blur** (SCRH-7) **and** after
 *   **10s** (CopyPaste-5917.56), independently. Here that drops the string, so
 *   an unattended screen is not merely covering a secret it still holds.
 *
 * Only one item is revealed at a time; revealing another drops the first. The
 * plaintext lives in component state and is never written to the query cache,
 * which would outlive the row and survive a re-render.
 */
import { useCallback, useEffect, useState } from "react";

import { classifyError, friendlyError } from "@/lib/errors";
import { REVEAL_TIMEOUT_MS } from "@/lib/layout";
import { revealItem } from "@/lib/ipc";

/**
 * `reveal_item` refuses on the desktop build today, and the refusal is
 * deliberate: the bridge's first attempt routed it through the `Copy` method,
 * which would have published the secret to the system pasteboard as a side
 * effect of *looking* at it. It stays refused until a read-only `Get` lands in
 * the wire contract.
 *
 * So a refusal is a normal state with its own sentence, not an error to be
 * reported. What the user needs to know is that the item is still usable — copy
 * puts it on the clipboard without its ever entering this window.
 */
const UNAVAILABLE_COPY =
  "Showing this item isn't available yet. You can still copy it — its contents never enter this window.";

interface Revealed {
  readonly id: string;
  readonly content: string;
}

export function useReveal() {
  const [revealed, setRevealed] = useState<Revealed | null>(null);
  const [pendingId, setPendingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const hide = useCallback(() => {
    setRevealed(null);
    setError(null);
  }, []);

  const reveal = useCallback(async (id: string) => {
    setPendingId(id);
    setError(null);
    try {
      const content = await revealItem(id);
      setRevealed({ id, content });
    } catch (raw) {
      setRevealed(null);
      // INV-12: a kind, never the raw text.
      const kind = classifyError(raw);
      setError(kind === "unavailable" ? UNAVAILABLE_COPY : friendlyError(kind));
    } finally {
      setPendingId(null);
    }
  }, []);

  useEffect(() => {
    if (revealed === null) return;

    const timer = setTimeout(() => setRevealed(null), REVEAL_TIMEOUT_MS);
    const onBlur = () => setRevealed(null);

    window.addEventListener("blur", onBlur);
    return () => {
      clearTimeout(timer);
      window.removeEventListener("blur", onBlur);
    };
  }, [revealed]);

  return {
    revealedId: revealed?.id ?? null,
    revealedContent: revealed?.content ?? null,
    pendingId,
    error,
    reveal,
    hide,
  };
}
