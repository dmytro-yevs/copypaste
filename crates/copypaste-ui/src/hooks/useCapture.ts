/**
 * One cache entry for capture state, written from three places: the query, the
 * push event, and every mutation's own return value. All three carry the whole
 * snapshot, so none of them has to guess what the others changed.
 *
 * The refresh on resume is a requirement rather than a nicety — v1 shipped a
 * cached permission state that never re-read after a trip to system Settings,
 * and on Android the grant genuinely disappears while the app is in the
 * background.
 */
import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";

import { HISTORY_KEY, STATUS_KEY } from "@/hooks/useHistory";
import { t } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import {
  type CaptureSnapshot,
  type CaptureSource,
  captureNow,
  captureRefresh,
  captureState,
  captureToastExplanation,
  hasBridge,
} from "@/lib/ipc";
import { useUi } from "@/store/ui";

export const CAPTURE_KEY = ["capture"] as const;
export const TOAST_EXPLANATION_KEY = ["capture", "toast-explanation"] as const;

/** Must match `capture::intake::EVENT_CAPTURE_STATE`, which has a test
 *  asserting it. */
export const EVENT_CAPTURE_STATE = "copypaste://capture-state";
/** Must match `capture::intake::EVENT_CAPTURED`. */
export const EVENT_CAPTURED = "copypaste://captured";

/**
 * Not polled. Every change to capture state is pushed — arming, binder death, a
 * background read landing — and a timer would only add traffic to a value that
 * announces itself. `useCaptureSync` re-reads on resume, which is the one
 * moment a push can have been missed.
 */
export function useCaptureState() {
  return useQuery<CaptureSnapshot>({
    queryKey: CAPTURE_KEY,
    queryFn: captureState,
    retry: false,
  });
}

/**
 * Mounted once, at the app root. A second subscriber would double every
 * invalidation, which is why `usePush` is mounted the same way.
 */
export function useCaptureSync() {
  const qc = useQueryClient();
  const setView = useUi((s) => s.setView);
  const snapshot = useCaptureState().data;

  useEffect(() => {
    if (!hasBridge()) return;

    let cancelled = false;
    const detach: Array<() => void> = [];

    function keep(unlisten: () => void) {
      if (cancelled) unlisten();
      else detach.push(unlisten);
    }

    void listen<CaptureSnapshot>(EVENT_CAPTURE_STATE, (event) => {
      qc.setQueryData(CAPTURE_KEY, event.payload);
    }).then(keep);

    void listen(EVENT_CAPTURED, () => {
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
      void qc.invalidateQueries({ queryKey: STATUS_KEY });
    }).then(keep);

    function refresh() {
      if (document.visibilityState !== "visible") return;
      void captureRefresh()
        .then((fresh) => qc.setQueryData(CAPTURE_KEY, fresh))
        // A bridge that will not answer is not evidence that the grant is
        // gone, so the last known snapshot stands. It is also not evidence
        // that capture works, and the model treats "no proof" as not working.
        .catch(() => {});
    }

    document.addEventListener("visibilitychange", refresh);
    window.addEventListener("focus", refresh);
    refresh();

    return () => {
      cancelled = true;
      for (const unlisten of detach) unlisten();
      document.removeEventListener("visibilitychange", refresh);
      window.removeEventListener("focus", refresh);
    };
  }, [qc]);

  // Read-and-cleared on the Kotlin side, so this is true on exactly the probe
  // that follows the notification tap. Re-arming is then one tap rather than a
  // hunt through settings.
  const rearm = snapshot?.rearmRequested ?? false;
  useEffect(() => {
    if (rearm) setView("capture");
  }, [rearm, setView]);
}

/**
 * Every capture command answers with the new snapshot, so one mutation covers
 * all of them and the cache is written from the value the backend just
 * computed rather than from what the caller hoped it would be.
 */
export function useCaptureMutation() {
  const qc = useQueryClient();
  return useMutation<CaptureSnapshot, unknown, () => Promise<CaptureSnapshot>>({
    mutationFn: (run) => run(),
    onSuccess: (fresh) => {
      qc.setQueryData(CAPTURE_KEY, fresh);
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** Rung 0, from a window that has focus. An empty clipboard is reported as
 *  such rather than as a success the user cannot find afterwards. */
export function useCaptureNow() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (source: CaptureSource) => captureNow(source),
    onSuccess: (item) => {
      if (item === null) {
        toast.info(t("capture.setup.always.nothing"));
        return;
      }
      toast.success(t("capture.setup.always.saved"));
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
      void qc.invalidateQueries({ queryKey: STATUS_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** Fetched only while the consent dialog is open: there is no reason to hold
 *  the text otherwise, and its arrival is what makes the confirm button
 *  legitimate. */
export function useToastExplanation(enabled: boolean) {
  return useQuery<string>({
    queryKey: TOAST_EXPLANATION_KEY,
    queryFn: captureToastExplanation,
    enabled,
    retry: false,
    staleTime: Infinity,
  });
}
