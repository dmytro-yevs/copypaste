import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";

import { t } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import { copyText, hasBridge } from "@/lib/ipc";
import { EVENT_CHANGED, type ChangePayload } from "@/hooks/usePush";
import {
  exportSupportBundle,
  getDiagnostics,
  type Diagnostics,
} from "@/service/diagnostics";

export const DIAGNOSTICS_KEY = ["diagnostics"] as const;

/** The change frame, plus the field only this panel reads. An intersection
 *  rather than a second declaration: `usePush` owns the shape. */
type SweepPayload = ChangePayload & { readonly swept: number };

/** A diagnostics panel is a live view. React Query starts it on mount and
 * tears the interval down with its last observer. */
export const DIAGNOSTICS_POLL_MS = 5_000;

export function useDiagnostics() {
  return useQuery<Diagnostics>({
    queryKey: DIAGNOSTICS_KEY,
    queryFn: getDiagnostics,
    refetchInterval: DIAGNOSTICS_POLL_MS,
    refetchIntervalInBackground: false,
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

/** Opens the system save flow. The backend builds the report again so the
 * exported file is redacted at its only source rather than trusting the page. */
export function useExportSupportBundle() {
  return useMutation<boolean, unknown, void>({
    mutationFn: exportSupportBundle,
    onSuccess: (saved) => {
      if (saved) toast.success(t("settings.diagnostics.report.exported"));
    },
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
