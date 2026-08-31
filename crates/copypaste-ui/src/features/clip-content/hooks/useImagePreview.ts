import { useQuery } from "@tanstack/react-query";

import { imagePreviewKey } from "@/lib/imagePreviewQuery";
import { getImagePreview, type ImagePreview } from "@/lib/ipc";

const MEDIA_GC_MS = 300_000;

export function useImagePreview(id: string) {
  return useQuery<ImagePreview>({
    queryKey: imagePreviewKey(id),
    queryFn: () => getImagePreview(id),
    staleTime: Infinity,
    gcTime: MEDIA_GC_MS,
    retry: false,
  });
}
