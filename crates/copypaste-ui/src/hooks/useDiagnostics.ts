/**
 * Reading the service's own account of itself, and announcing the one thing it
 * does that nobody asked for.
 */
import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";

import { t } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import { copyText, hasBridge } from "@/lib/ipc";
import { EVENT_CHANGED, type ChangePayload } from "@/hooks/usePush";
import { getDiagnostics, type Diagnostics } from "@/service/diagnostics";

export const DIAGNOSTICS_KEY = ["diagnostics"] as const;

/** The change frame, plus the field only this panel reads. An intersection
 *  rather than a second declaration: `usePush` owns the shape. */
type SweepPayload = ChangePayload & { readonly swept: number };

/**
 * Polled, unlike every other settings query: the counters and the uptime are
 * the point, and a panel a user opened because something is wrong must not show
 * them the state from when they opened it.
 */
const POLL_MS = 5_000;

export function useDiagnostics() {
  return useQuery<Diagnostics>({
    queryKey: DIAGNOSTICS_KEY,
    queryFn: getDiagnostics,
    refetchInterval: POLL_MS,
    retry: false,
  });
}

/**
 * The report goes to the clipboard through the backend, like any other text the
 * WebView already holds — the clipboard plugin's `writeText` is withheld by
 * `capabilities/default.json`.
 */
export function useCopyReport() {
  return useMutation<void, unknown, string>({
    mutationFn: (report) => copyText(report),
    onSuccess: () => toast.success(t("settings.diagnostics.report.copied")),
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/**
 * Announce an auto-deletion as it happens.
 *
 * The sweep is the only history change the user did not ask for, and until the
 * change event carried `swept` there was no way to tell them: they would find an
 * item missing and have nothing to attribute it to. A count, never the rows —
 * they are gone, and the event carries no content.
 *
 * A running total is on the panel regardless of whether this window was open,
 * because a notice nobody was there to see is not a record.
 */
export function useSweepNotices() {
  const qc = useQueryClient();

  useEffect(() => {
    if (!hasBridge()) return;

    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void listen<SweepPayload>(EVENT_CHANGED, (event) => {
      const swept = event.payload.swept;
      if (swept > 0) {
        toast.warning(t("settings.diagnostics.swept", { count: swept }));
        void qc.invalidateQueries({ queryKey: DIAGNOSTICS_KEY });
      }
    }).then((off) => {
      // An effect that unmounts before the promise settles must still detach,
      // or a fast remount stacks two listeners and doubles every notice.
      if (cancelled) off();
      else unlisten = off;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [qc]);
}
