import type { ComponentProps } from "react";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import styles from "./control-adornment.module.css";

const adornmentVariants = cva(styles.root, {
  variants: {
    size: { compact: styles.compact, regular: styles.regular },
    tone: { inherit: styles.inherit, muted: styles.muted },
  },
  defaultVariants: { size: "regular", tone: "inherit" },
});

type AdornmentVariants = VariantProps<typeof adornmentVariants>;

export function ControlAdornment({
  className,
  size,
  tone,
  children,
  ...props
}: ComponentProps<"span"> & AdornmentVariants) {
  return (
    <span
      data-slot="control-adornment"
      className={cn(adornmentVariants({ size, tone }), className)}
      {...props}
    >
      {children}
    </span>
  );
}
