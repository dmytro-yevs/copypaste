import type { ReactNode } from "react";

import { HelpPopover } from "./HelpPopover";
import { StateView, type StateMode } from "./StateView";
import { Button, Icon, type IconName } from "@/components/ui";

export type EmptyStateTone = "neutral" | "info" | "attention" | "danger" | "private";
export type EmptyStateIcon = IconName;

interface EmptyStateProps {
  icon?: EmptyStateIcon;
  busy?: boolean;
  tone?: EmptyStateTone;
  title: string;
  body?: string;
  details?: ReactNode;
  detailsLabel?: string;
  action?: { label: string; onClick: () => void; icon?: EmptyStateIcon; disabled?: boolean };
  secondary?: ReactNode;
  compact?: boolean;
  fullWidth?: boolean;
}

const mode: Record<EmptyStateTone, StateMode> = {
  neutral: "empty", info: "info", attention: "warning", danger: "error", private: "empty",
};

/** Compatibility adapter while feature callers move to StateView. */
export function EmptyState({ icon, busy = false, tone = "neutral", title, body, details, detailsLabel = "More information", action, secondary, compact = false, fullWidth = false }: EmptyStateProps) {
  const actions = action || secondary ? <>
    {action ? <Button variant="secondary" disabled={action.disabled} onClick={action.onClick}>{action.icon ? <Icon name={action.icon} size="sm" /> : null}{action.label}</Button> : null}
    {secondary}
  </> : undefined;
  return <StateView
    mode={busy ? "loading" : mode[tone]}
    placement={compact || fullWidth ? "panel" : "screen"}
    icon={icon}
    title={<>{title}{details ? <HelpPopover content={details} label={detailsLabel} /> : null}</>}
    description={body}
    actions={actions}
    role={busy ? "status" : tone === "danger" ? "alert" : "none"}
  />;
}
