import type { ReactNode } from "react";

import type { IconName } from "@/components/ui/icon";
import { StateView, type StateMode } from "./StateView";

export type StatusCardStatus = "positive" | "info" | "attention" | "danger" | "neutral" | "off";

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

const mode: Record<StatusCardStatus, StateMode> = {
  positive: "success", info: "info", attention: "warning", danger: "error", neutral: "info", off: "offline",
};

/** Compatibility adapter while feature callers move to StateView. */
export function StatusCard({ status, title, detail, meta, icon, action, density = "regular", variant = "standard", role = "status", live = "polite", atomic, busy = false, "aria-label": ariaLabel }: StatusCardProps) {
  return <StateView
    mode={busy ? "loading" : mode[status]}
    placement="panel"
    title={title}
    description={detail || meta ? <>{detail}{meta ? <small>{meta}</small> : null}</> : undefined}
    icon={icon}
    actions={action}
    data-slot="status-card"
    data-status={status}
    data-density={density}
    data-variant={variant}
    role={role}
    aria-live={live}
    aria-atomic={atomic}
    aria-busy={busy || undefined}
    aria-label={ariaLabel}
  />;
}
