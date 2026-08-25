import { cn } from "@/lib/cn";
import brandMarkUrl from "../../../src-tauri/icons/copypaste.svg";
import styles from "./BrandMark.module.css";

export function BrandMark({ label = "CopyPaste", size = "sidebar", animated = false }: { label?: string; size?: "app" | "sidebar" | "mono"; animated?: boolean }) {
  return (
    <svg
      className={cn(styles.mark, styles[size], animated && styles.animated)}
      role="img"
      aria-label={label}
    >
      <image href={brandMarkUrl} width="100%" height="100%" />
    </svg>
  );
}
