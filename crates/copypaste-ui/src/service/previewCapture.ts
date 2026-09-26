import type { CaptureSnapshot } from "@/generated/ipc";

const DESKTOP_CAPTURE_SNAPSHOT = {
  rung: "desktop",
  health: { state: "working" },
  shizuku: {
    supported: false,
    installed: false,
    running: false,
    permission: false,
    enabled: false,
    toastSuppressed: false,
    rearmRequested: false,
  },
  nextStep: "none",
  headline: "Clipboard capture is running.",
  detail: null,
  lastReadOkAt: null,
  lastCaptureAt: null,
  droppedClips: 0,
  toastSuppressed: false,
  toastAcknowledged: true,
  rearmRequested: false,
} satisfies CaptureSnapshot;

const ANDROID_CAPTURE_SNAPSHOT = {
  rung: "shizuku",
  health: { state: "working" },
  shizuku: {
    supported: true,
    installed: true,
    running: true,
    permission: true,
    enabled: true,
    toastSuppressed: false,
    rearmRequested: false,
  },
  nextStep: "none",
  headline: "Background capture is active.",
  detail: "Copies from other apps are being saved on this phone.",
  lastReadOkAt: Date.now(),
  lastCaptureAt: Date.now() - 90_000,
  droppedClips: 0,
  toastSuppressed: false,
  toastAcknowledged: false,
  rearmRequested: false,
} satisfies CaptureSnapshot;

export function previewCaptureSnapshot(android: boolean): CaptureSnapshot {
  return android ? ANDROID_CAPTURE_SNAPSHOT : DESKTOP_CAPTURE_SNAPSHOT;
}
