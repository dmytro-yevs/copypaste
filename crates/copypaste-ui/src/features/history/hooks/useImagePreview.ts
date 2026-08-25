import { useQuery } from "@tanstack/react-query";

import {
  getImagePreview,
  type ImagePreview,
} from "@/lib/ipc";
import { imagePreviewKey } from "@/features/history/model/imagePreviewQuery";

const MEDIA_GC_MS = 300_000;

/** A clipping's bytes never change under its id, so the query stays fresh
 * until finite garbage collection releases the cached payload. */
export function useImagePreview(id: string) {
  return useQuery<ImagePreview>({
    queryKey: imagePreviewKey(id),
    queryFn: () => getImagePreview(id),
    staleTime: Infinity,
    gcTime: MEDIA_GC_MS,
    retry: false,
  });
}
