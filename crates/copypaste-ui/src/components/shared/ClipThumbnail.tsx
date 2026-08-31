import type { ReactNode } from "react";

import { cn } from "@/lib/cn";
import type { Kind } from "@/lib/format";
import styles from "./ClipThumbnail.module.css";

export function ClipThumbnail({ image }: { kind: Extract<Kind, "image">; image?: ReactNode }) {
  return <div className={cn(styles.root, styles.image)}>{image}</div>;
}
