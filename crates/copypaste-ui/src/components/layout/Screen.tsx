import type { ComponentProps } from "react";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import styles from "./Screen.module.css";

const screenVariants = cva(styles.root, {
  variants: { height: { full: styles.full, content: styles.content } },
  defaultVariants: { height: "full" },
});

export function Screen({ className, height, ...props }: ComponentProps<"section"> & VariantProps<typeof screenVariants>) {
  return <section data-slot="screen" className={cn(screenVariants({ height }), className)} {...props} />;
}
