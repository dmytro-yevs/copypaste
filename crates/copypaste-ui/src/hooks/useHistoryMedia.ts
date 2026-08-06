/**
 * Both of these are fetched per *row*, and the virtualizer unmounts a row the
 * moment it leaves the overscan window — deliberately, so its Blob URL is
 * released. Local `useState` made that unmount destroy the fetch as well, so
 * scrolling back re-issued the IPC call, the base64 across two JSON hops and
 * the atob + Blob allocation. React Query de-duplicates concurrent identical
 * keys, so twenty rows from one application are one call.
 *
 * `gcTime` is finite on purpose: a thumbnail of a clipping must not outlive the
 * clipping. `useHistory`'s delete paths drop the entry outright.
 */
import { useQuery } from "@tanstack/react-query";

import {
  type ImagePreview,
  type SourceAppIcon,
  getImagePreview,
  getSourceAppIcon,
} from "@/lib/ipc";
import { ICON_STALE_MS, MEDIA_GC_MS } from "@/lib/layout";

export const SOURCE_APP_ICON_KEY = ["source-app-icon"] as const;
export const IMAGE_PREVIEW_KEY = ["image-preview"] as const;

export const imagePreviewKey = (id: string) =>
  [...IMAGE_PREVIEW_KEY, id] as const;

export function useSourceAppIcon(bundleId: string | null) {
  return useQuery<SourceAppIcon | null>({
    queryKey: [...SOURCE_APP_ICON_KEY, bundleId] as const,
    queryFn: () => getSourceAppIcon(bundleId as string),
    enabled: bundleId !== null && bundleId.length > 0,
    staleTime: ICON_STALE_MS,
    gcTime: MEDIA_GC_MS,
    retry: false,
  });
}

/** A clipping's bytes never change under its id, so this is only ever fetched
 *  once per item; `gcTime` is what bounds it. */
export function useImagePreview(id: string) {
  return useQuery<ImagePreview>({
    queryKey: imagePreviewKey(id),
    queryFn: () => getImagePreview(id),
    staleTime: Infinity,
    gcTime: MEDIA_GC_MS,
    retry: false,
  });
}
