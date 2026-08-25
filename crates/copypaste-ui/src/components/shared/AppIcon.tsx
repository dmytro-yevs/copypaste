import type { ComponentType } from "react";

import { cn } from "@/lib/cn";
import styles from "./AppIcon.module.css";
import { usePngObjectUrl } from "./usePngObjectUrl";

type FallbackIcon = ComponentType<{ size?: number; className?: string; "aria-hidden"?: boolean }>;

export function AppIcon({ pngBase64, Fallback, fallbackText, size = "sm", shape = "square", className }: { pngBase64?: string | null; Fallback?: FallbackIcon; fallbackText?: string; size?: "xs" | "sm" | "md"; shape?: "square" | "rounded" | "circle"; className?: string }) {
  const image = usePngObjectUrl(pngBase64);

  return (
    <span aria-hidden="true" className={cn(styles.root, styles[size], styles[shape], className)}>
      {image.state === "ready" ? <img src={image.url} alt="" draggable={false} className={styles.image} onError={image.invalidate} /> : fallbackText ? fallbackText : Fallback ? <Fallback size={11} aria-hidden={true} className={styles.glyph} /> : null}
    </span>
  );
}
