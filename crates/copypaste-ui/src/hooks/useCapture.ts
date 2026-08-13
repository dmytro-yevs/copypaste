/**
 * One cache entry for capture state, written by the query, the push event and
 * every mutation. All three carry the whole snapshot, so none has to guess what
 * the others changed.
 *
 * The refresh on resume is a requirement: on Android the grant disappears while
 * the app is backgrounded, and v1 shipped a cached permission state that never
 * re-read after a trip to system Settings.
 */
import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";

import { invalidateHistoryQueries, STATUS_KEY } from "@/hooks/useHistory";
import { t } from "@/i18n";
import { isRetryable, toFriendly } from "@/lib/errors";
import {
  type CaptureSnapshot,
  type CaptureSource,
  captureNow,
  captureRefresh,
  captureState,
  captureToastExplanation,
  hasBridge,
} from "@/lib/ipc";
import { EVENT_CAPTURED, EVENT_CAPTURE_STATE } from "@/lib/tauriEvents";
import { useUi } from "@/store/ui";

export { EVENT_CAPTURED, EVENT_CAPTURE_STATE } from "@/lib/tauriEvents";

export const CAPTURE_KEY = ["capture"] as const;
export const TOAST_EXPLANATION_KEY = ["capture", "toast-explanation"] as const;

/** Matches the Tauri bridge's own `DEFAULT_READ_TIMEOUT` (port manifest
 *  §3.4): `capture_state` is not on the long-read allowlist, so the frontend
 *  should not wait any longer than the backend already bounds it to. Left
 *  unbounded, a startup query that never answers held Android's navigation
 *  disabled for the JS-side `DEFAULT_TIMEOUT_MS` default of five minutes
 *  (DMY-136) — well past the release gate's 240 s observation window. */
const CAPTURE_STATE_STARTUP_TIMEOUT_MS = 10_000;
const CAPTURE_STATE_STARTUP_MAX_RETRIES = 5;

/** Not polled: every change to capture state is pushed — arming, binder death,
 *  a background read landing. `useCaptureSync` re-reads on resume, the one
 *  moment a push can have been missed.
 *
 *  A failed read still gets a bounded, causal retry: `retryable` distinguishes
 *  a transient failure (timeout, offline, `not_ready`) worth another attempt
 *  from a permanent one (auth, protocol) that must surface immediately rather
 *  than leave Android's startup navigation disabled. */
export function useCaptureState() {
  return useQuery<CaptureSnapshot>({
    queryKey: CAPTURE_KEY,
    queryFn: () => captureState({ timeoutMs: CAPTURE_STATE_STARTUP_TIMEOUT_MS }),
    retry: (failureCount, error) =>
      isRetryable(error) && failureCount < CAPTURE_STATE_STARTUP_MAX_RETRIES,
  });
}

/** Mounted once, at the app root: a second subscriber doubles every
 *  invalidation. */
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
      void invalidateHistoryQueries(qc);
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

/** Every capture command answers with the new snapshot, so the cache is written
 *  from what the backend computed rather than from what the caller hoped. */
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
      void invalidateHistoryQueries(qc);
      void qc.invalidateQueries({ queryKey: STATUS_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** Fetched only while the consent dialog is open: its arrival is what makes the
 *  confirm button legitimate. */
export function useToastExplanation(enabled: boolean) {
  return useQuery<string>({
    queryKey: TOAST_EXPLANATION_KEY,
    queryFn: captureToastExplanation,
    enabled,
    retry: false,
    staleTime: Infinity,
  });
}
