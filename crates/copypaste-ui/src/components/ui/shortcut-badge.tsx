import type { ComponentProps } from "react";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import styles from "./shortcut-badge.module.css";

const shortcutBadgeVariants = cva(styles.root, {
  variants: {
    size: {
      compact: styles.compact,
      regular: styles.regular,
    },
  },
  defaultVariants: { size: "regular" },
});

export function ShortcutBadge({
  className,
  size,
  children,
  ...props
}: ComponentProps<"kbd"> & VariantProps<typeof shortcutBadgeVariants>) {
  const parts = typeof children === "string"
    ? children.match(/^([⌘⌥⇧⌃]+)(.+)$/u)
    : null;

  return (
    <kbd
      data-slot="shortcut-badge"
      className={cn(shortcutBadgeVariants({ size }), className)}
      {...props}
    >
      {parts ? (
        <>
          <span className={styles.modifier}>{parts[1]}</span>
          <span className={styles.key}>{parts[2]}</span>
        </>
      ) : children}
    </kbd>
  );
}
