import { useQuery } from "@tanstack/react-query";

import {
  getSourceAppIcon,
  type SourceAppIcon,
} from "@/lib/ipc";

const ICON_STALE_MS = 300_000;
const ICON_GC_MS = 300_000;

export const SOURCE_APP_ICON_KEY = ["source-app-icon"] as const;

export function useSourceAppIcon(bundleId: string | null) {
  return useQuery<SourceAppIcon | null>({
    queryKey: [...SOURCE_APP_ICON_KEY, bundleId] as const,
    queryFn: () => getSourceAppIcon(bundleId as string),
    enabled: bundleId !== null && bundleId.length > 0,
    staleTime: ICON_STALE_MS,
    gcTime: ICON_GC_MS,
    retry: false,
  });
}
