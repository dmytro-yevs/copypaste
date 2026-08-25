import type { ComponentProps } from "react";
import { Slot } from "@radix-ui/react-slot";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import styles from "./surface.module.css";

export const surfaceVariants = cva(styles.surface, {
  variants: {
    elevation: {
      flat: styles.flat,
      raised: styles.raised,
      overlay: styles.overlay,
    },
    border: {
      none: styles.borderNone,
      subtle: styles.borderSubtle,
      strong: styles.borderStrong,
    },
    radius: { sm: styles.radiusSm, md: styles.radiusMd, lg: styles.radiusLg },
    tone: {
      neutral: styles.neutral,
      accent: styles.accent,
      warning: styles.warning,
      danger: styles.danger,
    },
  },
  defaultVariants: {
    elevation: "raised",
    border: "subtle",
    radius: "md",
    tone: "neutral",
  },
});

export type SurfaceVariants = VariantProps<typeof surfaceVariants>;

export function Surface({
  className,
  elevation,
  border,
  radius,
  tone,
  asChild = false,
  ...props
}: ComponentProps<"div"> & SurfaceVariants & { asChild?: boolean }) {
  const Comp = asChild ? Slot : "div";
  return (
    <Comp
      data-slot="surface"
      className={cn(surfaceVariants({ elevation, border, radius, tone }), className)}
      {...props}
    />
  );
}
