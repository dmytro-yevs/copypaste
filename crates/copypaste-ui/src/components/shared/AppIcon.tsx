import { useEffect, useState, type ComponentType } from "react";

import { cn } from "@/lib/cn";
import styles from "./AppIcon.module.css";

type FallbackIcon = ComponentType<{ size?: number; className?: string; "aria-hidden"?: boolean }>;

function pngUrl(base64?: string | null): string | null {
  if (!base64) return null;
  try {
    const binary = atob(base64);
    const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
    return URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
  } catch {
    return null;
  }
}

export function AppIcon({ pngBase64, Fallback, fallbackText, size = "sm", shape = "square", className }: { pngBase64?: string | null; Fallback?: FallbackIcon; fallbackText?: string; size?: "xs" | "sm" | "md"; shape?: "square" | "rounded" | "circle"; className?: string }) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    const objectUrl = pngUrl(pngBase64);
    setUrl(objectUrl);
    return () => { if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [pngBase64]);

  return (
    <span aria-hidden="true" className={cn(styles.root, styles[size], styles[shape], className)}>
      {url ? <img src={url} alt="" draggable={false} className={styles.image} onError={() => setUrl(null)} /> : fallbackText ? fallbackText : Fallback ? <Fallback size={11} aria-hidden={true} className={styles.glyph} /> : null}
    </span>
  );
}
