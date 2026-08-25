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
  detail?: string | null;
  meta?: string | null;
  icon?: IconName;
  action?: ReactNode;
  density?: "regular" | "compact";
  role?: "status" | "alert";
  live?: "polite" | "assertive";
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
  role = "status",
  live = "polite",
  busy = false,
  "aria-label": ariaLabel,
}: StatusCardProps) {
  return (
    <Surface asChild elevation="raised" border="subtle" radius="md">
      <section
        data-slot="status-card"
        data-status={status}
        data-density={density}
        className={styles.root}
        role={role}
        aria-live={live}
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
            {detail ? <small>{detail}</small> : null}
            {meta ? <small className={styles.meta}>{meta}</small> : null}
          </span>
          {action ? <span className={styles.action}>{action}</span> : null}
        </span>
      </section>
    </Surface>
  );
}
