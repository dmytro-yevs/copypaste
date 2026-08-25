import { useCallback, useEffect, useRef } from "react";

/**
 * The compact ladder has two levels and the system Back gesture is the only way
 * most Android users leave the second: `WryActivity.kt` routes Back to
 * `WebView.goBack()` while `canGoBack()`, so a subpage must be a real history
 * entry or Back closes the app from inside Settings.
 *
 * The entry exists exactly while the level does. The level also goes away with
 * no `popstate` behind it — a rotation across the size boundary, or leaving
 * Settings with a subpage open — and an entry that outlives it is not inert:
 * the next Back is spent traversing it and appears to do nothing.
 */
export function useSettingsLevel(open: boolean, close: () => void): () => void {
  const pushed = useRef(false);

  useEffect(() => {
    if (!open) return;
    // No URL: a URL change is a navigation the asset protocol would answer.
    window.history.pushState({ copypasteSettingsSubpage: true }, "");
    pushed.current = true;
    return () => {
      // A `popstate` close has already spent the entry and cleared the flag, so
      // this is the path where nothing else did.
      if (!pushed.current) return;
      pushed.current = false;
      window.history.back();
    };
  }, [open]);

  useEffect(() => {
    const onPop = () => {
      // An entry this hook did not push, or one it already spent: a traversal
      // started at unmount lands after the listener that started it is gone.
      if (!pushed.current) return;
      pushed.current = false;
      close();
    };
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, [close]);

  return useCallback(() => {
    // `history.back()` closes the subpage through `popstate`, so the entry is
    // spent rather than left behind for a later Back press to spend on nothing.
    if (pushed.current) window.history.back();
    else close();
  }, [close]);
}
