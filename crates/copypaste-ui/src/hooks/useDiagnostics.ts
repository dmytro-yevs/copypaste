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

/** Polled, unlike every other settings query: a panel opened because something
 *  is wrong must not show the state from when it was opened. */
const POLL_MS = 5_000;

export function useDiagnostics() {
  return useQuery<Diagnostics>({
    queryKey: DIAGNOSTICS_KEY,
    queryFn: getDiagnostics,
    refetchInterval: POLL_MS,
    retry: false,
  });
}

/** Through the backend: the clipboard plugin's `writeText` is withheld by
 *  `capabilities/default.json`. */
export function useCopyReport() {
  return useMutation<void, unknown, string>({
    mutationFn: (report) => copyText(report),
    onSuccess: () => toast.success(t("settings.diagnostics.report.copied")),
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/**
 * The sweep is the only history change the user did not ask for: without a
 * notice they find an item missing and have nothing to attribute it to. A
 * count, never the rows — the event carries no content.
 *
 * The panel keeps a running total whether or not this window was open, because
 * a notice nobody saw is not a record.
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
