import { useCallback, useEffect, useRef } from "react";

/**
 * The compact settings ladder has two levels, and the system Back gesture is
 * the only way most Android users leave the second one. `WryActivity.kt` routes
 * Back to `WebView.goBack()` whenever `canGoBack()` is true, so a subpage has
 * to be a real history entry or Back closes the app from inside Settings.
 *
 * `pushState` is called with no URL: the app is a single document served from
 * one origin, and a URL change is a navigation the asset protocol would have to
 * answer.
 */
export function useSettingsLevel(open: boolean, close: () => void): () => void {
  const pushed = useRef(false);

  useEffect(() => {
    if (!open || pushed.current) return;
    window.history.pushState({ copypasteSettingsSubpage: true }, "");
    pushed.current = true;
  }, [open]);

  useEffect(() => {
    const onPop = () => {
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
