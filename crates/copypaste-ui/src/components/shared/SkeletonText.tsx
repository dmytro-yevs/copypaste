import { cn } from "@/lib/cn";
import { StateView } from "./StateView";
import styles from "./SkeletonText.module.css";

export type SkeletonTextWidth = "xs" | "sm" | "md" | "fill";

/** Compatibility adapter; all loading visuals now use StateView's single spinner. */
export function SkeletonText({ width = "sm", className }: { width?: SkeletonTextWidth; className?: string }) {
  return <StateView mode="loading" placement="control" role="none" aria-hidden="true" data-width={width} className={cn(styles.root, className)} />;
}
