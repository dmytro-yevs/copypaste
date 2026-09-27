import type { ReactNode } from "react";

import { Icon, type IconName, Surface } from "@/components/ui";
import styles from "./StatusCard.module.css";

export type StatusCardStatus =
  | "positive"
  | "info"
  | "attention"
  | "danger"
  | "neutral"
  | "off";

export interface StatusCardProps {
  status: StatusCardStatus;
  title: string;
  detail?: ReactNode;
  meta?: string | null;
  icon?: IconName;
  action?: ReactNode;
  density?: "regular" | "compact";
  variant?: "standard" | "prominent";
  role?: "status" | "alert";
  live?: "polite" | "assertive";
  atomic?: boolean;
  busy?: boolean;
  "aria-label"?: string;
}

export function StatusCard({
  status,
  title,
  detail,
  meta,
  icon,
  action,
  density = "regular",
  variant = "standard",
  role = "status",
  live = "polite",
  atomic,
  busy = false,
  "aria-label": ariaLabel,
}: StatusCardProps) {
  return (
    <Surface asChild elevation="raised" border="subtle" radius="md">
      <section
        data-slot="status-card"
        data-status={status}
        data-density={density}
        data-variant={variant}
        className={styles.root}
        role={role}
        aria-live={live}
        aria-atomic={atomic}
        aria-busy={busy || undefined}
        aria-label={ariaLabel}
      >
        <span className={styles.layout}>
          <span className={styles.indicator} aria-hidden="true">
            {icon ? (
              <Icon name={icon} size="sm" />
            ) : (
              <span className={styles.dot} />
            )}
          </span>
          <span className={styles.copy}>
            <strong>{title}</strong>
            {detail ? <span className={styles.detail}>{detail}</span> : null}
            {meta ? <small className={styles.meta}>{meta}</small> : null}
          </span>
          {action ? <span className={styles.action}>{action}</span> : null}
        </span>
      </section>
    </Surface>
  );
}
