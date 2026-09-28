import type { ReactNode } from "react";
import type { IconName } from "@/components/ui/icon";

import { StateView, type StateMode } from "./StateView";

export interface InlineNoticeProps {
  children: ReactNode;
  tone?: "neutral" | "accent" | "warning" | "danger";
  icon?: IconName;
  action?: ReactNode;
  role?: "alert" | "status";
  live?: boolean;
}

const mode: Record<NonNullable<InlineNoticeProps["tone"]>, StateMode> = {
  neutral: "info", accent: "info", warning: "warning", danger: "error",
};

/** Compatibility adapter while feature callers move to StateView. */
export function InlineNotice({ children, tone = "neutral", icon, action, role, live = false }: InlineNoticeProps) {
  return <StateView mode={mode[tone]} placement="inline" title={children} icon={icon} actions={action} role={role ?? (live ? "status" : "none")} aria-live={live && role === undefined ? "polite" : undefined} />;
}
