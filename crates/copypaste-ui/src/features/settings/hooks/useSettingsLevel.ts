import { useCallback, useEffect, useRef } from "react";

/** WryActivity routes Android Back through WebView history. Each compact detail
 * needs an entry; unspent entries must be removed on rotation or leaving
 * Settings, or a later Back appears to do nothing. URLs stay unchanged because
 * the asset protocol would treat a URL change as navigation. */
export function useSettingsLevel(depth: number, onBack: () => void): () => void {
  const pushed = useRef(0);
  const ignoreNextPop = useRef(false);

  useEffect(() => {
    if (depth > pushed.current) {
      for (let level = pushed.current; level < depth; level += 1) {
        window.history.pushState({ copypasteSettingsSubpage: level + 1 }, "");
      }
      pushed.current = depth;
    } else if (depth < pushed.current) {
      // Programmatic collapse emits popstate but must not run user Back again.
      ignoreNextPop.current = true;
      window.history.go(depth - pushed.current);
      pushed.current = depth;
    }
  }, [depth]);

  useEffect(() => {
    const onPop = () => {
      if (ignoreNextPop.current) {
        ignoreNextPop.current = false;
        return;
      }
      if (pushed.current === 0) return;
      pushed.current -= 1;
      onBack();
    };
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, [onBack]);

  useEffect(() => () => {
    if (pushed.current === 0) return;
    window.history.go(-pushed.current);
    pushed.current = 0;
  }, []);

  return useCallback(() => {
    if (pushed.current > 0) window.history.back();
    else onBack();
  }, [onBack]);
}
