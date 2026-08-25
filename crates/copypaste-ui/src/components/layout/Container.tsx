import type { ComponentProps } from "react";
import { Slot } from "@radix-ui/react-slot";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import styles from "./Container.module.css";

const containerVariants = cva(styles.root, {
  variants: {
    width: { fluid: styles.fluid, reading: styles.reading, library: styles.library },
    gutter: { none: styles.gutterNone, compact: styles.gutterCompact, screen: styles.gutterScreen },
  },
  defaultVariants: { width: "fluid", gutter: "screen" },
});

export function Container({ className, width, gutter, asChild = false, ...props }: ComponentProps<"div"> & VariantProps<typeof containerVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot : "div";
  return <Comp className={cn(containerVariants({ width, gutter }), className)} {...props} />;
}
