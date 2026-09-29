import type { OnboardingPermissionStatus } from "@/lib/ipc";

export type PermissionAction = "request" | "open-settings" | "none";
export type PermissionLabel =
  | "request"
  | "granted"
  | "open-settings"
  | "not-required"
  | "unavailable";
export interface PermissionPresentation {
  readonly action: PermissionAction;
  readonly label: PermissionLabel;
  readonly disabled: boolean;
}

const PRESENTATION = {
  prompt: {
    action: "request",
    label: "request",
    disabled: false,
  },
  granted: {
    action: "none",
    label: "granted",
    disabled: true,
  },
  denied: {
    action: "open-settings",
    label: "open-settings",
    disabled: false,
  },
  not_required: {
    action: "none",
    label: "not-required",
    disabled: true,
  },
  unavailable: {
    action: "none",
    label: "unavailable",
    disabled: true,
  },
} as const satisfies Record<OnboardingPermissionStatus, PermissionPresentation>;

export function permissionPresentation(
  status: OnboardingPermissionStatus,
): PermissionPresentation {
  return PRESENTATION[status];
}
