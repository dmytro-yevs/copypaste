import type { ComponentProps } from "react";
import { type VariantProps, cva } from "class-variance-authority";

import { Surface, type SurfaceVariants } from "@/components/ui";
import { cn } from "@/lib/cn";
import styles from "./PreviewSurface.module.css";

const previewVariants = cva(styles.root, {
  variants: {
    padding: { none: styles.none, compact: styles.compact, regular: styles.regular, roomy: styles.roomy },
    scroll: { true: styles.scroll, false: undefined },
  },
  defaultVariants: { padding: "regular", scroll: false },
});

export function PreviewSurface({ className, padding, scroll, ...props }: ComponentProps<typeof Surface> & SurfaceVariants & VariantProps<typeof previewVariants>) {
  return <Surface data-slot="preview-surface" className={cn(previewVariants({ padding, scroll }), className)} {...props} />;
}
