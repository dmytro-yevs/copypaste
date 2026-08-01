import { useEffect, useState, type CSSProperties } from "react";
import { ImageOff, LoaderCircle } from "lucide-react";

import { getImagePreview } from "@/lib/ipc";
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
  const [state, setState] = useState<"loading" | "ready" | "failed">("loading");
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    let objectUrl: string | null = null;
    void getImagePreview(id)
      .then((preview) => {
        objectUrl = pngUrl(preview.png_base64);
        if (active) {
          setUrl(objectUrl);
          setState(objectUrl === null ? "failed" : "ready");
        }
        else if (objectUrl !== null) URL.revokeObjectURL(objectUrl);
      })
      .catch(() => {
        if (active) {
          setUrl(null);
          setState("failed");
        }
      });
    return () => {
      active = false;
      if (objectUrl !== null) URL.revokeObjectURL(objectUrl);
    };
  }, [id]);

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
