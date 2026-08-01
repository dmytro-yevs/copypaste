import { useEffect, useState, type ComponentType } from "react";

import { getSourceAppIcon } from "@/lib/ipc";
import { cn } from "@/lib/cn";

type FallbackIcon = ComponentType<{ size?: number; className?: string; "aria-hidden"?: boolean }>;

interface SourceAppIconProps {
  bundleId: string | null;
  Fallback: FallbackIcon;
  className?: string;
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

/**
 * A source id is all React receives. Native code resolves and bounds the icon,
 * then this component owns the short-lived Blob URL rather than an app path.
 */
export function SourceAppIcon({ bundleId, Fallback, className }: SourceAppIconProps) {
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    // Do not keep a previous application's logo in a recycled virtual row
    // while the next icon is resolving.
    setUrl(null);
    if (!bundleId) {
      return;
    }
    let active = true;
    let objectUrl: string | null = null;
    void getSourceAppIcon(bundleId)
      .then((icon) => {
        objectUrl = icon ? pngUrl(icon.png_base64) : null;
        if (active) setUrl(objectUrl);
        else if (objectUrl) URL.revokeObjectURL(objectUrl);
      })
      .catch(() => {
        if (active) setUrl(null);
      });
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [bundleId]);

  if (url) {
    return (
      <img
        data-source-app-icon
        src={url}
        alt=""
        draggable={false}
        className={cn("size-4 shrink-0 object-contain", className)}
        onError={() => setUrl(null)}
      />
    );
  }
  return <Fallback size={11} aria-hidden={true} className={cn("shrink-0", className)} />;
}
