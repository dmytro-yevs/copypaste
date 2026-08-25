import type { ComponentProps } from "react";
import { Slot } from "@radix-ui/react-slot";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import styles from "./Stack.module.css";

const stackVariants = cva(styles.root, {
  variants: {
    gap: {
      none: styles.gapNone,
      xs: styles.gapXs,
      sm: styles.gapSm,
      md: styles.gapMd,
      lg: styles.gapLg,
      xl: styles.gapXl,
    },
    align: {
      stretch: styles.stretch,
      start: styles.start,
      center: styles.center,
      end: styles.end,
    },
  },
  defaultVariants: { gap: "md", align: "stretch" },
});

export function Stack({ className, gap, align, asChild = false, ...props }: ComponentProps<"div"> & VariantProps<typeof stackVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot : "div";
  return <Comp className={cn(stackVariants({ gap, align }), className)} {...props} />;
}
