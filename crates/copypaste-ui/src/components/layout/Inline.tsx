import type { ComponentProps } from "react";
import { Slot } from "@radix-ui/react-slot";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import styles from "./Inline.module.css";

const inlineVariants = cva(styles.root, {
  variants: {
    gap: { none: styles.gapNone, xs: styles.gapXs, sm: styles.gapSm, md: styles.gapMd, lg: styles.gapLg },
    align: { start: styles.start, center: styles.center, end: styles.end, stretch: styles.stretch },
    justify: { start: styles.justifyStart, center: styles.justifyCenter, end: styles.justifyEnd, between: styles.justifyBetween },
    wrap: { true: styles.wrap, false: styles.nowrap },
  },
  defaultVariants: { gap: "md", align: "center", justify: "start", wrap: false },
});

export function Inline({ className, gap, align, justify, wrap, asChild = false, ...props }: ComponentProps<"div"> & VariantProps<typeof inlineVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot : "div";
  return <Comp className={cn(inlineVariants({ gap, align, justify, wrap }), className)} {...props} />;
}
