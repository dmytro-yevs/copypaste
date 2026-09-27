import { ClipImage, type ClipImageProps } from "@/components/shared";
import { useImagePreview } from "@/features/clip-content/hooks/useImagePreview";

export function ClipImageLoader({
  id,
  ...props
}: Omit<ClipImageProps, "pngBase64" | "loading" | "failed"> & { id: string }) {
  const cssEdge = props.size === "detail" || props.size === "fill" ? 1_024 : 384;
  const dpr = typeof window === "undefined" ? 1 : window.devicePixelRatio || 1;
  const maxEdge = Math.min(2_048, Math.max(128, Math.round(cssEdge * dpr / 64) * 64));
  const preview = useImagePreview(id, maxEdge);
  return <ClipImage {...props} pngBase64={preview.data?.png_base64 ?? null} loading={preview.isPending} failed={preview.isError} />;
}
