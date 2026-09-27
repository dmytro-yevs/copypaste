import type { ReactNode } from "react";

import { ActionButton } from "./ActionButton";
import { HelpPopover } from "./HelpPopover";
import { cn } from "@/lib/cn";
import { Icon, type IconName } from "@/components/ui/icon";
import { Surface } from "@/components/ui";
import styles from "./EmptyState.module.css";

export type EmptyStateTone =
    "neutral" | "info" | "attention" | "danger" | "private";
export type EmptyStateIcon = IconName;

interface EmptyStateProps {
    icon?: EmptyStateIcon;
    busy?: boolean;
    tone?: EmptyStateTone;
    title: string;
    body?: string;
    details?: ReactNode;
    detailsLabel?: string;
    action?: {
        label: string;
        onClick: () => void;
        icon?: EmptyStateIcon;
        disabled?: boolean;
    };
    secondary?: ReactNode;
    compact?: boolean;
    fullWidth?: boolean;
}

export function EmptyState({
    icon,
    busy = false,
    tone = "neutral",
    title,
    body,
    details,
    detailsLabel = "More information",
    action,
    secondary,
    compact = false,
    fullWidth = false,
}: EmptyStateProps) {
    return (
        <Surface asChild elevation="flat" border="none" radius="md">
          <section
              className={cn(
                  styles.root,
                  compact && styles.compact,
                  fullWidth && styles.fullWidth,
              )}
              data-tone={tone}
              role={busy ? "status" : tone === "danger" ? "alert" : undefined}
              aria-live={busy ? "polite" : undefined}
              aria-busy={busy || undefined}
          >
            <span aria-hidden="true" className={styles.marker}>
                {busy ? (
                    <Icon name="spinner" className={styles.spinner} size="md" />
                ) : icon ? (
                    <Icon name={icon} size="md" />
                ) : null}
            </span>
            <div className={styles.content}>
                <div className={styles.copy}>
                    <div className={styles.titleRow}>
                        <p className={styles.title}>{title}</p>
                        {details ? <HelpPopover content={details} label={detailsLabel} /> : null}
                    </div>
                    {body ? <p className={styles.body}>{body}</p> : null}
                </div>
                {action || secondary ? (
                    <div className={styles.actions}>
                      <div className={styles.actionLayout}>
                      {action ? (
                        <ActionButton
                            disabled={action.disabled}
                            onClick={action.onClick}
                            icon={action.icon}
                        >
                            {action.label}
                        </ActionButton>
                      ) : null}
                      {secondary ? (
                        <div className={styles.secondary}>{secondary}</div>
                      ) : null}
                      </div>
                    </div>
                ) : null}
            </div>
          </section>
        </Surface>
    );
}
