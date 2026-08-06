import { useEffect, useState, type ComponentType } from "react";

import { useSourceAppIcon } from "@/hooks/useHistoryMedia";
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
  const icon = useSourceAppIcon(bundleId);
  const base64 = icon.data?.png_base64 ?? null;
  const [url, setUrl] = useState<string | null>(null);

  // The URL, not the fetch, is what this component owns: it is revoked when the
  // virtualizer recycles the row, and never kept from a previous application
  // while the next icon resolves.
  useEffect(() => {
    if (base64 === null) {
      setUrl(null);
      return;
    }
    const objectUrl = pngUrl(base64);
    setUrl(objectUrl);
    return () => {
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [base64]);

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
