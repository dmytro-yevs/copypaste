import type { HTMLAttributes, ReactNode } from "react";

import { Icon, type IconName } from "@/components/ui/icon";
import { cn } from "@/lib/cn";
import styles from "./StateView.module.css";

export type StateMode = "loading" | "empty" | "offline" | "error" | "warning" | "info" | "success";
export type StatePlacement = "control" | "inline" | "panel" | "screen";

export interface StateViewProps extends Omit<HTMLAttributes<HTMLElement>, "children" | "title"> {
  mode: StateMode;
  placement?: StatePlacement;
  title?: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  icon?: IconName;
}

const defaultIcon: Record<Exclude<StateMode, "loading">, IconName> = {
  empty: "searchX",
  offline: "plug",
  error: "alert",
  warning: "alert",
  info: "info",
  success: "checkCircle",
};

/** Shared state presentation. Feature state resolution and recovery remain with callers. */
export function StateView({
  mode,
  placement = "panel",
  title,
  description,
  actions,
  icon,
  className,
  role,
  ...attributes
}: StateViewProps) {
  const resolvedRole = role ?? (mode === "error" ? "alert" : "status");
  const resolvedIcon = mode === "loading" ? "spinner" : icon ?? defaultIcon[mode];
  const content = (
    <>
      <span className={styles.marker} aria-hidden="true">
        <Icon name={resolvedIcon} size={placement === "control" ? "xs" : "sm"} />
      </span>
      {(title != null || description != null) && (
        <span className={styles.copy}>
          {title != null && <span className={styles.title}>{title}</span>}
          {description != null && <span className={styles.description}>{description}</span>}
        </span>
      )}
      {actions != null && <span className={styles.actions}>{actions}</span>}
    </>
  );
  const shared = {
    ...attributes,
    className: cn(styles.root, className),
    "data-mode": mode,
    "data-placement": placement,
    role: resolvedRole,
    "aria-live": attributes["aria-live"] ?? (resolvedRole === "alert" ? "assertive" : resolvedRole === "status" ? "polite" : undefined),
    "aria-busy": attributes["aria-busy"] ?? (mode === "loading" || undefined),
  };

  return placement === "control" || placement === "inline"
    ? <span {...shared}>{content}</span>
    : <section {...shared}>{content}</section>;
}
