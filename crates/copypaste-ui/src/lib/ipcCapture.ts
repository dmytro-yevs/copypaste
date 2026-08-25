import type {
  CapturedPayload as GeneratedCapturedPayload,
  CaptureHealth as GeneratedCaptureHealth,
  CaptureNextStep as GeneratedCaptureNextStep,
  CaptureRung as GeneratedCaptureRung,
  CaptureSnapshot as GeneratedCaptureSnapshot,
  CaptureSource as GeneratedCaptureSource,
  NotGrantedReason as GeneratedNotGrantedReason,
  NotWorkingReason as GeneratedNotWorkingReason,
  ShizukuProbe as GeneratedShizukuProbe,
} from "@/generated/ipc";
import { UI_COMMANDS } from "@/generated/ipc";
import type { ReadonlyDeep } from "type-fest";
import type { Item } from "./ipc";
import { call, hasWebBridge, type IpcCallOptions } from "./ipcCall";
import { isAndroidPlatform } from "./platform";

export type CapturedPayload = ReadonlyDeep<GeneratedCapturedPayload>;
export type CaptureHealth = ReadonlyDeep<GeneratedCaptureHealth>;
export type CaptureNextStep = GeneratedCaptureNextStep;
export type CaptureRung = GeneratedCaptureRung;
export type CaptureSnapshot = ReadonlyDeep<GeneratedCaptureSnapshot>;
export type CaptureSource = GeneratedCaptureSource;
export type NotGrantedReason = GeneratedNotGrantedReason;
export type NotWorkingReason = GeneratedNotWorkingReason;
export type ShizukuProbe = ReadonlyDeep<GeneratedShizukuProbe>;

const WEB_BRIDGE_CAPTURE_SNAPSHOT: CaptureSnapshot = {
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
};

const WEB_BRIDGE_ANDROID_CAPTURE_SNAPSHOT: CaptureSnapshot = {
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
};

function webBridgeCaptureSnapshot(): CaptureSnapshot {
  return isAndroidPlatform()
    ? WEB_BRIDGE_ANDROID_CAPTURE_SNAPSHOT
    : WEB_BRIDGE_CAPTURE_SNAPSHOT;
}

export function captureState(options?: IpcCallOptions): Promise<CaptureSnapshot> {
  if (hasWebBridge()) return Promise.resolve(webBridgeCaptureSnapshot());
  return call<CaptureSnapshot>(UI_COMMANDS.capture_state, undefined, options);
}

/** Called on every resume, not only at startup: a grant can lapse while the app
 *  is backgrounded, and a reboot is the ordinary case. */
export function captureRefresh(): Promise<CaptureSnapshot> {
  if (hasWebBridge()) return Promise.resolve(webBridgeCaptureSnapshot());
  return call<CaptureSnapshot>(UI_COMMANDS.capture_refresh);
}

/** One call for two steps: it asks for the permission when that is what is
 *  missing, and starts the background reader when it is not. */
export function captureArm(): Promise<CaptureSnapshot> {
  if (hasWebBridge()) return Promise.resolve(webBridgeCaptureSnapshot());
  return call<CaptureSnapshot>(UI_COMMANDS.capture_arm);
}

export function captureDisarm(): Promise<CaptureSnapshot> {
  if (hasWebBridge()) return Promise.resolve(webBridgeCaptureSnapshot());
  return call<CaptureSnapshot>(UI_COMMANDS.capture_disarm);
}

export function captureSetEnabled(enabled: boolean): Promise<CaptureSnapshot> {
  if (hasWebBridge()) {
    const snapshot = webBridgeCaptureSnapshot();
    return Promise.resolve({
      ...snapshot,
      health: enabled ? snapshot.health : { state: "disabled" },
      shizuku: { ...snapshot.shizuku, enabled },
    });
  }
  return call<CaptureSnapshot>(UI_COMMANDS.capture_set_enabled, { enabled });
}

/** Rung 0: save whatever is on the clipboard right now. `null` means there was
 *  nothing to save, which is not a failure. */
export function captureNow(source: CaptureSource): Promise<Item | null> {
  if (hasWebBridge()) return Promise.resolve(null);
  return call<Item | null>(UI_COMMANDS.capture_now, { source });
}

/** The exact text `authorise_toast` gates on. Fetched rather than copied into
 *  the catalogue: a second copy is a second thing to keep true. */
export function captureToastExplanation(): Promise<string> {
  if (hasWebBridge()) {
    return Promise.resolve(
      "Android shows a privacy notice when an app reads the clipboard. Hiding it affects the whole device, not only CopyPaste.",
    );
  }
  return call<string>(UI_COMMANDS.capture_toast_explanation);
}

/** `acknowledged` may only be `true` when the user has read
 *  `captureToastExplanation` and agreed to it — passing `true` without having
 *  shown the text lies to a gate Rust enforces. Turning suppression **off** is
 *  never gated. */
export function captureSetToastSuppressed(
  suppressed: boolean,
  acknowledged: boolean,
): Promise<CaptureSnapshot> {
  if (hasWebBridge()) {
    const snapshot = webBridgeCaptureSnapshot();
    return Promise.resolve({
      ...snapshot,
      toastSuppressed: suppressed,
      toastAcknowledged: acknowledged,
      shizuku: { ...snapshot.shizuku, toastSuppressed: suppressed },
    });
  }
  return call<CaptureSnapshot>(UI_COMMANDS.capture_set_toast_suppressed, {
    suppressed,
    acknowledged,
  });
}

export function captureOpenShizuku(): Promise<void> {
  if (hasWebBridge()) return Promise.resolve();
  return call<void>(UI_COMMANDS.capture_open_shizuku);
}

export function captureOpenDeveloperOptions(): Promise<void> {
  if (hasWebBridge()) return Promise.resolve();
  return call<void>(UI_COMMANDS.capture_open_developer_options);
}

export function captureRequestBatteryExemption(): Promise<void> {
  if (hasWebBridge()) return Promise.resolve();
  return call<void>(UI_COMMANDS.capture_request_battery_exemption);
}
