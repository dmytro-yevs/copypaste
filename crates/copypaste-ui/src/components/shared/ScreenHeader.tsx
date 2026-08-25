import type { ReactNode } from "react";

import { PaneHeader } from "@/components/layout";
import { cn } from "@/lib/cn";
import styles from "./ScreenHeader.module.css";

export function ScreenHeader({ eyebrow, title, description, actions, leading, className }: { eyebrow?: ReactNode; title: ReactNode; description?: ReactNode; actions?: ReactNode; leading?: ReactNode; className?: string }) {
  return (
    <PaneHeader
      className={cn(styles.root, className)}
      layoutClassName={styles.layout}
    >
      {leading ? <div className={styles.leading}>{leading}</div> : null}
      <div className={styles.copy}>
        {eyebrow ? <p className={styles.eyebrow}>{eyebrow}</p> : null}
        <h1 className={styles.title}>{title}</h1>
        {description ? <p className={styles.description}>{description}</p> : null}
      </div>
      {actions ? <div className={styles.actions}>{actions}</div> : null}
    </PaneHeader>
  );
}
