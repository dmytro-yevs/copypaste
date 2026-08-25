import { permissionSnapshot } from "@/lib/ipcPermissions";
import { hasNativeBridge } from "@/lib/ipcCall";

export type AppPlatform =
  | "macos"
  | "windows"
  | "android"
  | "linux"
  | "browser"
  | "unknown";

const NATIVE_PLATFORMS = new Set<AppPlatform>([
  "macos",
  "windows",
  "android",
  "linux",
]);

let platform: AppPlatform = "unknown";

function browserPreviewPlatform(): AppPlatform {
  if (typeof window === "undefined") return "browser";
  const requested = new URLSearchParams(window.location.search).get("platform");
  return NATIVE_PLATFORMS.has(requested as AppPlatform)
    ? (requested as AppPlatform)
    : "browser";
}

export async function initializePlatform(): Promise<AppPlatform> {
  if (!hasNativeBridge()) {
    platform = browserPreviewPlatform();
    return platform;
  }
  try {
    const snapshot = await permissionSnapshot();
    platform = NATIVE_PLATFORMS.has(snapshot.platform)
      ? snapshot.platform
      : "unknown";
  } catch {
    platform = "__TAURI_INTERNALS__" in window
      ? "unknown"
      : browserPreviewPlatform();
  }
  return platform;
}

export function currentPlatform(): AppPlatform {
  return platform === "unknown" && typeof window !== "undefined" && !hasNativeBridge()
    ? browserPreviewPlatform()
    : platform;
}

export function isAndroid(value: string = currentPlatform()): boolean {
  return value === "android";
}

export function isAndroidPlatform(): boolean {
  return isAndroid();
}

export function isWindows(value: string = currentPlatform()): boolean {
  return value === "windows";
}

export function isWindowsPlatform(): boolean {
  return isWindows();
}
