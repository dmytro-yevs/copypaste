import { useQuery } from "@tanstack/react-query";

import { imagePreviewKey } from "@/lib/imagePreviewQuery";
import { getImagePreview, type ImagePreview } from "@/lib/ipc";

const MEDIA_GC_MS = 300_000;

export function useImagePreview(id: string, maxEdge: number, enabled = true) {
  return useQuery<ImagePreview>({
    queryKey: imagePreviewKey(id, maxEdge),
    queryFn: () => getImagePreview(id, maxEdge),
    staleTime: Infinity,
    gcTime: MEDIA_GC_MS,
    retry: false,
    enabled,
  });
}
