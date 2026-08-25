import type { ReactNode } from "react";

import { ActionButton } from "./ActionButton";
import { cn } from "@/lib/cn";
import { Icon, type IconName } from "@/components/ui/icon";
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
    action?: {
        label: string;
        onClick: () => void;
        icon?: EmptyStateIcon;
        disabled?: boolean;
    };
    secondary?: ReactNode;
    secondaryPlacement?: "attached" | "separated";
    compact?: boolean;
}

export function EmptyState({
    icon,
    busy = false,
    tone = "neutral",
    title,
    body,
    action,
    secondary,
    secondaryPlacement = "separated",
    compact = false,
}: EmptyStateProps) {
    return (
        <section
            className={cn(styles.root, compact && styles.compact)}
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
                    <p className={styles.title}>{title}</p>
                    {body ? <p className={styles.body}>{body}</p> : null}
                </div>
                {action ? (
                    <div className={styles.action}>
                        <ActionButton
                            disabled={action.disabled}
                            onClick={action.onClick}
                            icon={action.icon}
                        >
                            {action.label}
                        </ActionButton>
                    </div>
                ) : null}
                {secondary ? (
                    <div
                        className={cn(
                            styles.secondary,
                            secondaryPlacement === "attached" &&
                                styles.attached,
                        )}
                    >
                        {secondary}
                    </div>
                ) : null}
            </div>
        </section>
    );
}
