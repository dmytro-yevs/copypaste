/**
 * Two properties this file keeps:
 *
 * A write carries only the field it changed, so two open screens — or a screen
 * and the CLI — cannot overwrite each other's unsaved work.
 *
 * A rejected write leaves the cache on what the service kept: the service stays
 * on the old record when a value is out of range, so failure invalidates rather
 * than patches, and the screen never shows the value that was refused.
 */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { t } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import {
  type ConfigApplied,
  type ConfigPatch,
  type ExportReport,
  type ImportPreview,
  type ImportReport,
  type ServiceState,
  applyImportHistory,
  backupDatabase,
  cancelImportHistory,
  exportHistory,
  getConfig,
  getPrivateMode,
  prepareImportHistory,
  restartService,
  restoreDatabase,
  setConfig,
  setPrivateMode,
  type PrivateModeData,
} from "@/lib/ipc";
import { HISTORY_KEY, STATUS_KEY } from "@/hooks/useHistory";

export const CONFIG_KEY = ["config"] as const;
export const PRIVATE_MODE_KEY = ["private-mode"] as const;

/** No poll. These change when this app or the CLI changes them, and a settings
 *  screen re-reading every few seconds would fight the control the user is
 *  holding. */
export function useServiceConfig() {
  return useQuery<ConfigApplied>({
    queryKey: CONFIG_KEY,
    queryFn: getConfig,
    retry: false,
  });
}

/** `restart_required` is passed to the caller rather than toasted: it has to
 *  stay on screen until the restart happens, and a toast is gone before the
 *  user looks back. */
export function useSetServiceConfig() {
  const qc = useQueryClient();
  return useMutation<ConfigApplied, unknown, ConfigPatch>({
    mutationFn: setConfig,
    onSuccess: (applied) => qc.setQueryData(CONFIG_KEY, applied),
    onError: (raw) => {
      toast.error(toFriendly(raw));
      void qc.invalidateQueries({ queryKey: CONFIG_KEY });
    },
  });
}

export function usePrivateMode() {
  return useQuery<PrivateModeData>({
    queryKey: PRIVATE_MODE_KEY,
    queryFn: getPrivateMode,
    retry: false,
  });
}

export function useSetPrivateMode() {
  const qc = useQueryClient();
  return useMutation<PrivateModeData, unknown, boolean>({
    mutationFn: setPrivateMode,
    onSuccess: (confirmed) => {
      qc.setQueryData(PRIVATE_MODE_KEY, confirmed);
      void qc.invalidateQueries({ queryKey: CONFIG_KEY });
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
      void qc.invalidateQueries({ queryKey: STATUS_KEY });
    },
    onError: (raw) => {
      toast.error(toFriendly(raw));
      void qc.invalidateQueries({ queryKey: PRIVATE_MODE_KEY });
    },
  });
}

/** Refuses for a service this app did not start (ADR-0004): that is an answer,
 *  not a failure to work around — whoever started it another way is the one who
 *  can restart it. */
export function useRestartService() {
  const qc = useQueryClient();
  return useMutation<ServiceState, unknown, void>({
    mutationFn: () => restartService(),
    onSuccess: () => {
      toast.success(t("settings.service.liveness.restarted"));
      void qc.invalidateQueries({ queryKey: CONFIG_KEY });
      void qc.invalidateQueries({ queryKey: STATUS_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** A count of zero is dropped rather than rendered: "Exported 12 items" with no
 *  trailing clause *is* the statement that nothing was left out. */
function summarise(parts: readonly (string | null)[]): string {
  return parts.filter((part): part is string => part !== null).join(" · ");
}

function exportSummary(report: ExportReport): string {
  return summarise([
    t("settings.transfer.export.done", { count: report.exported }),
    report.skipped_sensitive > 0
      ? t("settings.transfer.export.withheld", { count: report.skipped_sensitive })
      : null,
    report.skipped_non_text > 0
      ? t("settings.transfer.export.nonText", { count: report.skipped_non_text })
      : null,
    report.skipped_undecryptable > 0
      ? t("settings.transfer.export.unreadable", {
          count: report.skipped_undecryptable,
        })
      : null,
  ]);
}

/** `null` means the user closed the save panel, which is not a failure and gets
 *  no toast. A withheld count raises the toast to a warning rather than adding
 *  a line to a success: it must not read as "done". */
export function useExportHistory() {
  return useMutation<ExportReport | null, unknown, boolean>({
    mutationFn: (includeSensitive) => exportHistory(includeSensitive),
    onSuccess: (report) => {
      if (report === null) return;
      const message = exportSummary(report);
      if (report.skipped_sensitive > 0 || report.skipped_undecryptable > 0) {
        toast.warning(message);
      } else {
        toast.success(message);
      }
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

export function useImportHistory() {
  const qc = useQueryClient();
  const prepare = useMutation<ImportPreview | null, unknown, void>({
    mutationFn: () => prepareImportHistory(),
    onError: (raw) => toast.error(toFriendly(raw)),
  });
  const apply = useMutation<ImportReport, unknown, string>({
    mutationFn: (token) => applyImportHistory(token),
    onSuccess: (report) => {
      toast.success(
        summarise([
          t("settings.transfer.import.done", { count: report.inserted }),
          report.skipped > 0
            ? t("settings.transfer.import.alreadyHere", { count: report.skipped })
            : null,
        ]),
      );
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
      void qc.invalidateQueries({ queryKey: STATUS_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
  const cancel = useMutation<void, unknown, string>({
    mutationFn: (token) => cancelImportHistory(token),
    onError: (raw) => toast.error(toFriendly(raw)),
  });
  return {
    prepare,
    apply,
    cancel,
    isPending: prepare.isPending || apply.isPending,
  };
}

/** Reported in megabytes, never in bytes: the number is reassurance that
 *  something real was written, and eight digits do not read as one. */
function megabytes(bytes: number): string {
  return Math.max(0.1, bytes / 1_048_576).toFixed(1);
}

export function useBackupDatabase() {
  return useMutation<number | null, unknown, void>({
    mutationFn: () => backupDatabase(),
    onSuccess: (size) => {
      if (size === null) return;
      toast.success(
        t("settings.transfer.backup.done", { megabytes: megabytes(size) }),
      );
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** Every cached query is dropped rather than invalidated: after a restore the
 *  ids in the cache belong to a history that no longer exists, and an
 *  invalidation would re-render the old rows until each refetch lands. */
export function useRestoreDatabase() {
  const qc = useQueryClient();
  return useMutation<boolean, unknown, void>({
    mutationFn: () => restoreDatabase(),
    onSuccess: (restored) => {
      if (!restored) return;
      toast.success(t("settings.transfer.restore.done"));
      qc.clear();
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}
