import type { ComponentProps, ReactNode } from "react";

import {
  PaneHeader,
  PaneHeaderActions,
  PaneHeaderCopy,
  ScrollViewport,
} from "@/components/layout";
import { cn } from "@/lib/cn";
import styles from "./InspectorShell.module.css";

export function InspectorShell({
  title,
  headerActions,
  actions,
  metadata,
  children,
  className,
  ...props
}: Omit<ComponentProps<"aside">, "title"> & {
  title: ReactNode;
  headerActions?: ReactNode;
  actions?: ReactNode;
  metadata?: ReactNode;
}) {
  return (
    <aside
      data-slot="inspector-shell"
      className={cn(styles.root, className)}
      {...props}
    >
      <div className={styles.layout}>
        <PaneHeader className={styles.header}>
          <PaneHeaderCopy className={styles.title}>{title}</PaneHeaderCopy>
          {headerActions ? (
            <PaneHeaderActions>{headerActions}</PaneHeaderActions>
          ) : null}
        </PaneHeader>
        <ScrollViewport padding="none" className={styles.body}>
          <div className={styles.content}>{children}</div>
          {actions ? (
            <div className={styles.actions}>
              <div className={styles.actionLayout}>{actions}</div>
            </div>
          ) : null}
          {metadata ? <div className={styles.metadata}>{metadata}</div> : null}
        </ScrollViewport>
      </div>
    </aside>
  );
}
