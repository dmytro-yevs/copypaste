import { ClipImage, type ClipImageProps } from "@/components/shared";
import { useImagePreview } from "@/features/clip-content/hooks/useImagePreview";
import { useObservedElementSize } from "@/hooks/useViewportMetrics";

export function ClipImageLoader({
  id,
  ...props
}: Omit<ClipImageProps, "pngBase64" | "loading" | "failed"> & { id: string }) {
  const observed = useObservedElementSize<HTMLElement>();
  const dpr = typeof window === "undefined" ? 1 : window.devicePixelRatio || 1;
  const maxEdge = previewEdge(observed.width, observed.height, dpr);
  const preview = useImagePreview(id, maxEdge);
  return <ClipImage {...props} measureRef={observed.ref} pngBase64={preview.data?.png_base64 ?? null} loading={preview.isPending} failed={preview.isError} />;
}

export function previewEdge(width: number, height: number, dpr: number): number {
  const cssEdge = Math.max(width, height, 1);
  return Math.min(2_048, Math.max(128, Math.ceil(cssEdge * dpr / 64) * 64));
}
