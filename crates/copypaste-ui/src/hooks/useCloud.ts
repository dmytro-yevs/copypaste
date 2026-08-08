import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { HISTORY_KEY } from "@/hooks/useHistory";
import { t } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import {
  type CloudCredentials,
  type CloudStatusData,
  type CloudSyncData,
  cloudSignIn,
  cloudSignOut,
  getCloudStatus,
  syncCloudNow,
} from "@/lib/ipc";

export const CLOUD_STATUS_KEY = ["cloud-status"] as const;

export function useCloudStatus() {
  return useQuery<CloudStatusData>({
    queryKey: CLOUD_STATUS_KEY,
    queryFn: getCloudStatus,
    refetchInterval: 10_000,
    retry: false,
  });
}

export function useCloudSignIn() {
  const qc = useQueryClient();
  return useMutation<CloudStatusData, unknown, CloudCredentials>({
    mutationFn: cloudSignIn,
    onSuccess: (status) => {
      qc.setQueryData(CLOUD_STATUS_KEY, status);
      toast.success(t("settings.sync.cloud.toast.signedIn"));
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

export function useCloudSignOut() {
  const qc = useQueryClient();
  return useMutation<CloudStatusData, unknown, void>({
    mutationFn: cloudSignOut,
    onSuccess: (status) => {
      qc.setQueryData(CLOUD_STATUS_KEY, status);
      toast.success(t("settings.sync.cloud.toast.signedOut"));
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

function withheld(result: CloudSyncData): number {
  return (
    result.skipped_sensitive +
    result.skipped_undecryptable +
    result.skipped_forged +
    result.skipped_future +
    result.skipped_too_large
  );
}

export function useCloudSyncNow() {
  const qc = useQueryClient();
  return useMutation<CloudSyncData, unknown, void>({
    mutationFn: syncCloudNow,
    onSuccess: (result) => {
      const skipped = withheld(result);
      const key = skipped === 0
        ? "settings.sync.cloud.toast.synced"
        : "settings.sync.cloud.toast.syncedWithSkips";
      toast[skipped === 0 ? "success" : "warning"](
        t(key, {
          uploaded: result.uploaded,
          downloaded: result.downloaded,
          skipped,
        }),
      );
      void qc.invalidateQueries({ queryKey: CLOUD_STATUS_KEY });
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}
