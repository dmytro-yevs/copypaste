import type {
  OnboardingPermissionId as GeneratedPermissionId,
  OnboardingPermissionItem as GeneratedPermissionItem,
  OnboardingPermissionStatus as GeneratedPermissionStatus,
  OnboardingPermissions as GeneratedOnboardingPermissions,
  PermissionHost as GeneratedPermissionHost,
} from "@/generated/ipc";
import type { ReadonlyDeep } from "type-fest";
import { call } from "./ipcCall";

export type PermissionHost = GeneratedPermissionHost;
export type OnboardingPermissionId = GeneratedPermissionId;
export type OnboardingPermissionStatus = GeneratedPermissionStatus;
export type OnboardingPermissionItem = ReadonlyDeep<GeneratedPermissionItem>;
export type OnboardingPermissions = ReadonlyDeep<GeneratedOnboardingPermissions>;

export function permissionSnapshot(): Promise<OnboardingPermissions> {
  return call<OnboardingPermissions>("permission_snapshot");
}

export function permissionRequest(
  id: OnboardingPermissionId,
): Promise<OnboardingPermissions> {
  return call<OnboardingPermissions>("permission_request", { id });
}

export function permissionOpenSettings(
  id: OnboardingPermissionId,
): Promise<OnboardingPermissions> {
  return call<OnboardingPermissions>("permission_open_settings", { id });
}
