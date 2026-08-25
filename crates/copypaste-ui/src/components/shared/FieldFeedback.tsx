import type { HTMLAttributes, ReactNode } from "react";

import { Icon, type IconName } from "@/components/ui/icon";
import { cn } from "@/lib/cn";
import styles from "./FieldFeedback.module.css";

export type FieldFeedbackState =
  | "pending"
  | "error"
  | "warning"
  | "neutral"
  | "success";

const stateIcon: Record<FieldFeedbackState, IconName> = {
  pending: "spinner",
  error: "alert",
  warning: "alert",
  neutral: "info",
  success: "checkCircle",
};

interface FieldFeedbackProps
  extends Omit<HTMLAttributes<HTMLSpanElement>, "children" | "role"> {
  state: FieldFeedbackState;
  children: ReactNode;
  announce?: boolean;
}

export function FieldFeedback({
  state,
  children,
  announce = state !== "neutral",
  className,
  ...props
}: FieldFeedbackProps) {
  const role = announce ? (state === "error" ? "alert" : "status") : undefined;

  return (
    <span
      {...props}
      role={role}
      aria-live={role === "alert" ? "assertive" : role === "status" ? "polite" : undefined}
      className={cn(styles.root, className)}
      data-state={state}
    >
      <Icon
        name={stateIcon[state]}
        size="xs"
        className={state === "pending" ? styles.spinner : undefined}
      />
      <span>{children}</span>
    </span>
  );
}
