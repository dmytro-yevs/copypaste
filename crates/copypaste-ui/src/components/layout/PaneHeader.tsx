import type { ComponentProps } from "react";

import { cn } from "@/lib/cn";
import styles from "./PaneHeader.module.css";

export function PaneHeader({
  className,
  layoutClassName,
  children,
  ...props
}: ComponentProps<"header"> & { layoutClassName?: string }) {
  return (
    <header className={cn(styles.root, className)} {...props}>
      <div
        className={cn(styles.layout, layoutClassName)}
        data-slot="pane-header-layout"
      >
        {children}
      </div>
    </header>
  );
}

export function PaneHeaderCopy({ className, ...props }: ComponentProps<"div">) {
  return <div className={cn(styles.copy, className)} {...props} />;
}

export function PaneHeaderActions({
  className,
  children,
  ...props
}: ComponentProps<"div">) {
  return (
    <div className={cn(styles.actions, className)} {...props}>
      <div className={styles.actionLayout}>{children}</div>
    </div>
  );
}
