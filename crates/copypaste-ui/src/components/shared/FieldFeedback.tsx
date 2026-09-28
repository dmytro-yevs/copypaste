import type { HTMLAttributes, ReactNode } from "react";

import { StateView, type StateMode } from "./StateView";

export type FieldFeedbackState = "pending" | "error" | "warning" | "neutral" | "success";

interface FieldFeedbackProps extends Omit<HTMLAttributes<HTMLSpanElement>, "children" | "role"> {
  state: FieldFeedbackState;
  children: ReactNode;
  announce?: boolean;
}

const mode: Record<FieldFeedbackState, StateMode> = {
  pending: "loading", error: "error", warning: "warning", neutral: "info", success: "success",
};

/** Compatibility adapter while feature callers move to StateView. */
export function FieldFeedback({ state, children, announce = state !== "neutral", ...props }: FieldFeedbackProps) {
  return <StateView {...props} data-state={state} mode={mode[state]} placement="control" title={children} role={announce ? (state === "error" ? "alert" : "status") : "none"} aria-live={announce ? (state === "error" ? "assertive" : "polite") : undefined} />;
}
