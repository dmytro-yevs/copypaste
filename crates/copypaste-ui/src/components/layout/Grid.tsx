import type { ComponentProps } from "react";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import styles from "./Grid.module.css";

const gridVariants = cva(styles.root, {
  variants: {
    columns: { 1: styles.one, 2: styles.two, 3: styles.three, 4: styles.four },
    minItemWidth: { control: styles.autoControl, card: styles.autoCard, pane: styles.autoPane },
    gap: { none: styles.gapNone, sm: styles.gapSm, md: styles.gapMd, lg: styles.gapLg, xl: styles.gapXl },
  },
  defaultVariants: { columns: 1, gap: "md" },
});

export function Grid({ className, columns, minItemWidth, gap, ...props }: ComponentProps<"div"> & VariantProps<typeof gridVariants>) {
  return <div className={cn(gridVariants({ columns: minItemWidth ? null : columns, minItemWidth, gap }), className)} {...props} />;
}
