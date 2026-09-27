import { ClipImage, type ClipImageProps } from "@/components/shared";
import { useImagePreview } from "@/features/clip-content/hooks/useImagePreview";
import { useObservedElementSize } from "@/hooks/useViewportMetrics";

export function ClipImageLoader({
  id,
  ...props
}: Omit<ClipImageProps, "pngBase64" | "loading" | "failed"> & { id: string }) {
  const observed = useObservedElementSize<HTMLElement>();
  const cssEdge = Math.max(observed.width, observed.height, 1);
  const dpr = typeof window === "undefined" ? 1 : window.devicePixelRatio || 1;
  const maxEdge = Math.min(2_048, Math.max(128, Math.ceil(cssEdge * dpr / 64) * 64));
  const preview = useImagePreview(id, maxEdge);
  return <ClipImage {...props} measureRef={observed.ref} pngBase64={preview.data?.png_base64 ?? null} loading={preview.isPending} failed={preview.isError} />;
}
