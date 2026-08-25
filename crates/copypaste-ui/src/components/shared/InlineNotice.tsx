import type { ReactNode } from "react";
import { Icon, type IconName } from "@/components/ui/icon";

import { Inline } from "@/components/layout";
import { Surface } from "@/components/ui";
import styles from "./InlineNotice.module.css";

export interface InlineNoticeProps {
  children: ReactNode;
  tone?: "neutral" | "accent" | "warning" | "danger";
  icon?: IconName;
  action?: ReactNode;
  role?: "alert" | "status";
  live?: boolean;
}

export function InlineNotice({
  children,
  tone = "neutral",
  icon,
  action,
  role,
  live = false,
}: InlineNoticeProps) {
  return (
    <Surface
      className={styles.root}
      elevation="flat"
      radius="sm"
      tone={tone}
      role={role}
      aria-live={live && role === undefined ? "polite" : undefined}
    >
      <Inline gap="sm" align="center" justify="between" wrap>
        <Inline asChild gap="sm" align="center">
          <span className={styles.message}>
            {icon ? <Icon name={icon} size="sm" className={styles.icon} /> : null}
            <span className={styles.content}>{children}</span>
          </span>
        </Inline>
        {action != null && (
          <span className={styles.action}>{action}</span>
        )}
      </Inline>
    </Surface>
  );
}
