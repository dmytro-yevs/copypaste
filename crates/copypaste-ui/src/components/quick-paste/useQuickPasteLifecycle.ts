import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type FocusEvent,
  type RefObject,
} from "react";
import { useQueryClient } from "@tanstack/react-query";

import { hideWindow, setAllowScreenshots } from "@/lib/ipc";
import { applyAppearance } from "@/lib/theme";
import { readPrefs } from "@/store/prefs";

export const QUICK_PASTE_QUERY_KEY = ["quick-paste", "items"] as const;

declare global {
  interface Window {
    __copypasteFreeMemory?: () => void;
  }
}

interface QuickPasteLifecycleOptions {
  searchRef: RefObject<HTMLInputElement | null>;
  clearLocalState: () => void;
}

export function useQuickPasteLifecycle({
  searchRef,
  clearLocalState,
}: QuickPasteLifecycleOptions) {
  const [previewLinesPopup, setPreviewLinesPopup] = useState(
    () => readPrefs().previewLinesPopup,
  );
  const [historyPreviewLines, setHistoryPreviewLines] = useState(
    () => readPrefs().previewLines,
  );
  const [holding, setHolding] = useState(true);
  const holdingRef = useRef(true);
  const cacheGeneration = useRef(0);
  const hideInFlight = useRef(false);
  const hideGuardTimer = useRef<number | null>(null);
  const queryClient = useQueryClient();

  const applyShownPrefs = useCallback(() => {
    const prefs = readPrefs();
    applyAppearance(prefs);
    setPreviewLinesPopup(prefs.previewLinesPopup);
    setHistoryPreviewLines(prefs.previewLines);
    void setAllowScreenshots(prefs.allowScreenshots).catch(() => {});
    window.setTimeout(() => searchRef.current?.focus(), 50);
  }, [searchRef]);

  const refreshForShow = useCallback(() => {
    applyShownPrefs();
    if (holdingRef.current) {
      // Cancel first so INV-33 gets a new read instead of joining one in flight.
      void queryClient.cancelQueries({ queryKey: QUICK_PASTE_QUERY_KEY });
      void queryClient.refetchQueries({ queryKey: QUICK_PASTE_QUERY_KEY });
      return;
    }
    holdingRef.current = true;
    setHolding(true);
  }, [applyShownPrefs, queryClient]);

  const releaseHiddenCache = useCallback(() => {
    holdingRef.current = false;
    setHolding(false);
    cacheGeneration.current += 1;
    clearLocalState();
    // A visible read must not repopulate the cache after the popup hides (INV-33).
    void queryClient.cancelQueries({ queryKey: QUICK_PASTE_QUERY_KEY });
    queryClient.removeQueries({ queryKey: QUICK_PASTE_QUERY_KEY });
  }, [clearLocalState, queryClient]);

  useEffect(() => {
    window.__copypasteFreeMemory = releaseHiddenCache;
    return () => {
      if (window.__copypasteFreeMemory === releaseHiddenCache) {
        delete window.__copypasteFreeMemory;
      }
    };
  }, [releaseHiddenCache]);

  useEffect(() => {
    // The enabled query owns the mount read; another refetch would double it.
    applyShownPrefs();
    const onVisibility = () => {
      if (document.visibilityState === "visible") refreshForShow();
      else releaseHiddenCache();
    };
    window.addEventListener("focus", refreshForShow);
    window.addEventListener("blur", releaseHiddenCache);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      if (hideGuardTimer.current !== null) window.clearTimeout(hideGuardTimer.current);
      window.removeEventListener("focus", refreshForShow);
      window.removeEventListener("blur", releaseHiddenCache);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [applyShownPrefs, refreshForShow, releaseHiddenCache]);

  const dismiss = useCallback(() => {
    if (hideInFlight.current) return;
    hideInFlight.current = true;
    releaseHiddenCache();
    void hideWindow().catch(() => {});
    hideGuardTimer.current = window.setTimeout(() => {
      hideInFlight.current = false;
      hideGuardTimer.current = null;
    }, 100);
  }, [releaseHiddenCache]);

  const dismissOnRootBlur = useCallback(
    (event: FocusEvent<HTMLElement>) => {
      if (event.currentTarget.contains(event.relatedTarget)) return;
      if (
        event.relatedTarget instanceof Element &&
        event.relatedTarget.closest("[data-sonner-toaster]") !== null
      ) {
        return;
      }
      dismiss();
    },
    [dismiss],
  );

  const currentCacheGeneration = useCallback(() => cacheGeneration.current, []);
  const isCacheGenerationCurrent = useCallback(
    (generation: number) => cacheGeneration.current === generation,
    [],
  );

  return {
    holding,
    previewLinesPopup,
    historyPreviewLines,
    dismiss,
    dismissOnRootBlur,
    currentCacheGeneration,
    isCacheGenerationCurrent,
  };
}
