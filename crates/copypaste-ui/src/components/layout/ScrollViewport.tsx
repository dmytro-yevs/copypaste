import type { ComponentProps } from "react";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import styles from "./ScrollViewport.module.css";

const viewportVariants = cva(styles.root, {
  variants: { padding: { none: styles.none, compact: styles.compact, screen: styles.screen } },
  defaultVariants: { padding: "none" },
});

export function ScrollViewport({ className, padding, focusable = false, tabIndex, ...props }: ComponentProps<"div"> & VariantProps<typeof viewportVariants> & { focusable?: boolean }) {
  return <div className={cn(viewportVariants({ padding }), className)} tabIndex={focusable ? (tabIndex ?? 0) : tabIndex} {...props} />;
}
