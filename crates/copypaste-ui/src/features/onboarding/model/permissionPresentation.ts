import type { OnboardingPermissionStatus } from "@/lib/ipc";

export type PermissionAction = "request" | "open-settings" | "none";
export type PermissionLabel =
  | "request"
  | "granted"
  | "open-settings"
  | "not-required"
  | "unavailable";
export type PermissionExplanation =
  | "default"
  | "denied"
  | "not-required"
  | "unavailable";

export interface PermissionPresentation {
  readonly action: PermissionAction;
  readonly label: PermissionLabel;
  readonly explanation: PermissionExplanation;
  readonly disabled: boolean;
}

const PRESENTATION = {
  prompt: {
    action: "request",
    label: "request",
    explanation: "default",
    disabled: false,
  },
  granted: {
    action: "none",
    label: "granted",
    explanation: "default",
    disabled: true,
  },
  denied: {
    action: "open-settings",
    label: "open-settings",
    explanation: "denied",
    disabled: false,
  },
  not_required: {
    action: "none",
    label: "not-required",
    explanation: "not-required",
    disabled: true,
  },
  unavailable: {
    action: "none",
    label: "unavailable",
    explanation: "unavailable",
    disabled: true,
  },
} as const satisfies Record<OnboardingPermissionStatus, PermissionPresentation>;

export function permissionPresentation(
  status: OnboardingPermissionStatus,
): PermissionPresentation {
  return PRESENTATION[status];
}
