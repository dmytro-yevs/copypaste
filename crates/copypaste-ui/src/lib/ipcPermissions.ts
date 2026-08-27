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

/** Permission reads are local platform probes, not user-driven operations.
 * Match the native short-read boundary so a stopped Android host cannot keep
 * permission UI pending for the generic five-minute IPC allowance. */
export const PERMISSION_SNAPSHOT_TIMEOUT_MS = 10_000;

type PermissionSnapshotPhase = "started" | "ready" | "failed";

function reportPermissionSnapshot(
  phase: PermissionSnapshotPhase,
  startedAt: number,
): void {
  console.info("[copypaste] permission snapshot", {
    phase,
    durationMs: Math.max(0, Date.now() - startedAt),
  });
}

function isAndroidWebPreview(): boolean {
  return hasWebBridge() &&
    new URLSearchParams(window.location.search).get("platform") === "android";
}

export function permissionSnapshot(): Promise<OnboardingPermissions> {
  if (isAndroidWebPreview()) return Promise.resolve(androidPreviewPermissions);
  const startedAt = Date.now();
  reportPermissionSnapshot("started", startedAt);
  return call(UI_COMMANDS.permission_snapshot, undefined, {
    timeoutMs: PERMISSION_SNAPSHOT_TIMEOUT_MS,
  }).then(
    (snapshot) => {
      reportPermissionSnapshot("ready", startedAt);
      return snapshot;
    },
    (failure: unknown) => {
      reportPermissionSnapshot("failed", startedAt);
      throw failure;
    },
  );
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
