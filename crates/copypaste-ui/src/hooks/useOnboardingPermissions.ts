import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  type OnboardingPermissionId,
  type OnboardingPermissions,
  permissionOpenSettings,
  permissionRequest,
  permissionSnapshot,
} from "@/lib/ipc";
import { hasNativeBridge } from "@/lib/ipcCall";
import { isAndroidPlatform } from "@/lib/platform";

export const ONBOARDING_PERMISSIONS_KEY = ["onboarding-permissions"] as const;

export function useOnboardingPermissions() {
  return useQuery<OnboardingPermissions>({
    queryKey: ONBOARDING_PERMISSIONS_KEY,
    queryFn: permissionSnapshot,
    enabled: hasNativeBridge() || isAndroidPlatform(),
    retry: false,
  });
}

export function usePermissionRequest() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: OnboardingPermissionId) => permissionRequest(id),
    onSuccess: (fresh) => qc.setQueryData(ONBOARDING_PERMISSIONS_KEY, fresh),
  });
}

export function usePermissionOpenSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: OnboardingPermissionId) => permissionOpenSettings(id),
    onSuccess: (fresh) => qc.setQueryData(ONBOARDING_PERMISSIONS_KEY, fresh),
  });
}
