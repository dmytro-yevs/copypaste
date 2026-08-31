import { ClipImage, type ClipImageProps } from "@/components/shared";
import { useImagePreview } from "@/features/clip-content/hooks/useImagePreview";

export function ClipImageLoader({
  id,
  ...props
}: Omit<ClipImageProps, "pngBase64" | "loading" | "failed"> & { id: string }) {
  const preview = useImagePreview(id);
  return <ClipImage {...props} pngBase64={preview.data?.png_base64 ?? null} loading={preview.isPending} failed={preview.isError} />;
}
