/**
 * Revealing is a *fetch*, not a CSS state: the item arrives with
 * `content: null` (INV-10). That makes INV-11's two expiries literal — the
 * string is dropped on window blur (SCRH-7) and after 10s (CopyPaste-5917.56),
 * independently — rather than covering a secret the window still holds.
 */
import { useCallback, useEffect, useState } from "react";

import { t } from "@/i18n";
import { classifyError, friendlyError } from "@/lib/errors";
import { REVEAL_TIMEOUT_MS } from "@/lib/layout";
import { type Item, revealItem } from "@/lib/ipc";
import { usePrefs } from "@/store/prefs";

interface Revealed {
  readonly id: string;
  readonly content: string;
}

export function useReveal() {
  const warnFirst = usePrefs((s) => s.warnBeforeReveal);
  const [revealed, setRevealed] = useState<Revealed | null>(null);
  const [pendingId, setPendingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);

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
      // `reveal_item` refuses on desktop deliberately: the bridge's first
      // attempt routed it through `Copy`, which would have published the
      // secret to the system pasteboard as a side effect of *looking* at it.
      // The refusal is a normal state with its own sentence.
      setError(
        kind === "unavailable"
          ? t("history.reveal.unavailable")
          : friendlyError(kind),
      );
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

  // n9gp / PG-34: the warning is a preference, default on, and the reveal
  // itself is one click away either way.
  const request = useCallback(
    (item: Item) => {
      if (warnFirst) setConfirming(item.id);
      else void reveal(item.id);
    },
    [reveal, warnFirst],
  );

  const confirm = useCallback(() => {
    if (confirming !== null) void reveal(confirming);
    setConfirming(null);
  }, [confirming, reveal]);

  return {
    revealedId: revealed?.id ?? null,
    revealedContent: revealed?.content ?? null,
    pendingId,
    error,
    /** Ask for an item, through the confirmation when the preference is on. */
    request,
    confirming: confirming !== null,
    confirm,
    cancel: useCallback(() => setConfirming(null), []),
    hide,
  };
}
