import { useEffect, useState, type CSSProperties } from "react";
import { ImageOff, LoaderCircle } from "lucide-react";

import { useImagePreview } from "@/hooks/useHistoryMedia";
import { cn } from "@/lib/cn";

interface HistoryImagePreviewProps {
  id: string;
  className?: string;
  style?: CSSProperties;
  title?: string;
  loadingLabel?: string;
  failureLabel?: string;
}

function pngUrl(base64: string): string | null {
  try {
    const binary = atob(base64);
    const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
    return URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
  } catch {
    return null;
  }
}

/** The virtual list unmounts this outside its visible window, which also drops
 * its Blob URL instead of retaining image data for the whole history. */
export function HistoryImagePreview({
  id,
  className,
  style,
  title,
  loadingLabel,
  failureLabel,
}: HistoryImagePreviewProps) {
  const preview = useImagePreview(id);
  const base64 = preview.data?.png_base64 ?? null;
  const [url, setUrl] = useState<string | null>(null);
  const [undecodable, setUndecodable] = useState(false);

  // The Blob URL stays owned here, so it is still revoked the moment the
  // virtualizer unmounts the row; only the round trip behind it is cached.
  useEffect(() => {
    if (base64 === null) {
      setUrl(null);
      setUndecodable(false);
      return;
    }
    const objectUrl = pngUrl(base64);
    setUrl(objectUrl);
    setUndecodable(objectUrl === null);
    return () => {
      if (objectUrl !== null) URL.revokeObjectURL(objectUrl);
    };
  }, [base64]);

  const state =
    url !== null
      ? "ready"
      : preview.isError || undecodable
        ? "failed"
        : "loading";

  if (state !== "ready" || url === null) {
    const fallbackStyle = {
      ...style,
      width: style?.width ?? style?.height ?? 32,
      height: style?.height ?? 32,
    };
    return (
      <span
        aria-label={state === "loading" ? loadingLabel : failureLabel}
        role={loadingLabel || failureLabel ? "status" : undefined}
        title={title}
        className={cn(
          "flex shrink-0 items-center justify-center overflow-hidden rounded-md border border-border bg-elevated text-muted-foreground",
          className,
        )}
        style={fallbackStyle}
      >
        {state === "loading" ? (
          <LoaderCircle size={14} aria-hidden="true" className="animate-spin" />
        ) : (
          <ImageOff size={14} aria-hidden="true" />
        )}
      </span>
    );
  }

  return (
    <img
      className={cn(
        "block max-w-full shrink-0 object-contain",
        className,
      )}
      src={url}
      alt=""
      title={title}
      draggable={false}
      style={style}
    />
  );
}
