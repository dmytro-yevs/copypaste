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

function isAndroidBuild(): boolean {
  return import.meta.env.VITE_ANDROID_BUILD === "1";
}

function browserPreviewPlatform(): AppPlatform {
  if (typeof window === "undefined") return "browser";
  const requested = new URLSearchParams(window.location.search).get("platform");
  return NATIVE_PLATFORMS.has(requested as AppPlatform)
    ? (requested as AppPlatform)
    : "browser";
}

export function initializePlatform(): AppPlatform | Promise<AppPlatform> {
  // The Android bundle is a maintained build capability, so it is stronger
  // evidence than a permission probe that may be unavailable after force-stop.
  // Resolve it before any component module reads currentPlatform().
  if (isAndroidBuild()) {
    platform = "android";
    return platform;
  }
  if (!hasNativeBridge()) {
    platform = browserPreviewPlatform();
    return platform;
  }
  return permissionSnapshot()
    .then((snapshot) => {
      platform = NATIVE_PLATFORMS.has(snapshot.platform)
        ? snapshot.platform
        : "unknown";
      return platform;
    })
    .catch(() => {
      platform = "__TAURI_INTERNALS__" in window
        ? "unknown"
        : browserPreviewPlatform();
      return platform;
    });
}

export function currentPlatform(): AppPlatform {
  if (isAndroidBuild()) return "android";
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
