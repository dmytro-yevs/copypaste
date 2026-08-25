import type {
  OnboardingPermissionId as GeneratedPermissionId,
  OnboardingPermissionItem as GeneratedPermissionItem,
  OnboardingPermissionStatus as GeneratedPermissionStatus,
  OnboardingPermissions as GeneratedOnboardingPermissions,
  PermissionHost as GeneratedPermissionHost,
} from "@/generated/ipc";
import { UI_COMMANDS } from "@/generated/ipc";
import type { ReadonlyDeep } from "type-fest";
import { call, hasWebBridge } from "./ipcCall";

export type PermissionHost = GeneratedPermissionHost;
export type OnboardingPermissionId = GeneratedPermissionId;
export type OnboardingPermissionStatus = GeneratedPermissionStatus;
export type OnboardingPermissionItem = ReadonlyDeep<GeneratedPermissionItem>;
export type OnboardingPermissions = ReadonlyDeep<GeneratedOnboardingPermissions>;

let androidPreviewPermissions: OnboardingPermissions = {
  platform: "android",
  notifications: { id: "notifications", status: "prompt", required: false },
  tile: { id: "tile", status: "prompt", required: false },
  clipboardStatus: "not_required",
};

function isAndroidWebPreview(): boolean {
  return hasWebBridge() &&
    new URLSearchParams(window.location.search).get("platform") === "android";
}

export function permissionSnapshot(): Promise<OnboardingPermissions> {
  if (isAndroidWebPreview()) return Promise.resolve(androidPreviewPermissions);
  return call(UI_COMMANDS.permission_snapshot);
}

export function permissionRequest(
  id: OnboardingPermissionId,
): Promise<OnboardingPermissions> {
  if (isAndroidWebPreview()) {
    androidPreviewPermissions = {
      ...androidPreviewPermissions,
      [id]: { ...androidPreviewPermissions[id], status: "granted" },
    };
    return Promise.resolve(androidPreviewPermissions);
  }
  return call(UI_COMMANDS.permission_request, { id });
}

export function permissionOpenSettings(
  id: OnboardingPermissionId,
): Promise<OnboardingPermissions> {
  if (isAndroidWebPreview()) return permissionRequest(id);
  return call(UI_COMMANDS.permission_open_settings, { id });
}
