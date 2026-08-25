import type { ComponentProps } from "react";

import { cn } from "@/lib/cn";
import styles from "./MetadataList.module.css";

export function MetadataList({
  className,
  density = "regular",
  ...props
}: ComponentProps<"dl"> & { density?: "compact" | "regular" }) {
  return (
    <dl
      data-slot="metadata-list"
      data-density={density}
      className={cn(styles.list, className)}
      {...props}
    />
  );
}

export function MetadataRow({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="metadata-row"
      className={cn(styles.row, className)}
      {...props}
    />
  );
}

export function MetadataLabel({ className, ...props }: ComponentProps<"dt">) {
  return (
    <dt
      data-slot="metadata-label"
      className={cn(styles.label, className)}
      {...props}
    />
  );
}

export function MetadataValue({
  className,
  overflow = "wrap",
  ...props
}: ComponentProps<"dd"> & { overflow?: "wrap" | "truncate" }) {
  return (
    <dd
      data-slot="metadata-value"
      data-overflow={overflow}
      className={cn(styles.value, className)}
      {...props}
    />
  );
}
