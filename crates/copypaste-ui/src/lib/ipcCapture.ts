import type {
  CapturedPayload as GeneratedCapturedPayload,
  CaptureHealth as GeneratedCaptureHealth,
  CaptureNextStep as GeneratedCaptureNextStep,
  CaptureRung as GeneratedCaptureRung,
  CaptureSetupInstructions as GeneratedCaptureSetupInstructions,
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
import { previewCaptureSnapshot } from "@/service/previewCapture";

export type CapturedPayload = ReadonlyDeep<GeneratedCapturedPayload>;
export type CaptureHealth = ReadonlyDeep<GeneratedCaptureHealth>;
export type CaptureNextStep = GeneratedCaptureNextStep;
export type CaptureRung = GeneratedCaptureRung;
export type CaptureSetupInstructions = ReadonlyDeep<GeneratedCaptureSetupInstructions>;
export type CaptureSnapshot = ReadonlyDeep<GeneratedCaptureSnapshot>;
export type CaptureSource = GeneratedCaptureSource;
export type NotGrantedReason = GeneratedNotGrantedReason;
export type NotWorkingReason = GeneratedNotWorkingReason;
export type ShizukuProbe = ReadonlyDeep<GeneratedShizukuProbe>;

function webBridgeCaptureSnapshot(): CaptureSnapshot {
  return previewCaptureSnapshot(isAndroidPlatform());
}

export function captureState(options?: IpcCallOptions): Promise<CaptureSnapshot> {
  if (hasWebBridge()) return Promise.resolve(webBridgeCaptureSnapshot());
  return call(UI_COMMANDS.capture_state, undefined, options);
}

/** Called on every resume, not only at startup: a grant can lapse while the app
 *  is backgrounded, and a reboot is the ordinary case. */
export function captureRefresh(): Promise<CaptureSnapshot> {
  if (hasWebBridge()) return Promise.resolve(webBridgeCaptureSnapshot());
  return call(UI_COMMANDS.capture_refresh);
}

/** One call for two steps: it asks for the permission when that is what is
 *  missing, and starts the background reader when it is not. */
export function captureArm(): Promise<CaptureSnapshot> {
  if (hasWebBridge()) return Promise.resolve(webBridgeCaptureSnapshot());
  return call(UI_COMMANDS.capture_arm);
}

export function captureDisarm(): Promise<CaptureSnapshot> {
  if (hasWebBridge()) return Promise.resolve(webBridgeCaptureSnapshot());
  return call(UI_COMMANDS.capture_disarm);
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
  return call(UI_COMMANDS.capture_set_enabled, { enabled });
}

/** Rung 0: save whatever is on the clipboard right now. `null` means there was
 *  nothing to save, which is not a failure. */
export function captureNow(source: CaptureSource): Promise<Item | null> {
  if (hasWebBridge()) return Promise.resolve(null);
  return call(UI_COMMANDS.capture_now, { source });
}

/** The exact text `authorise_toast` gates on. Fetched rather than copied into
 *  the catalogue: a second copy is a second thing to keep true. */
export function captureToastExplanation(): Promise<string> {
  if (hasWebBridge()) {
    return Promise.resolve(
      "Android shows a privacy notice when an app reads the clipboard. Hiding it affects the whole device, not only CopyPaste.",
    );
  }
  return call(UI_COMMANDS.capture_toast_explanation);
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
  return call(UI_COMMANDS.capture_set_toast_suppressed, {
    suppressed,
    acknowledged,
  });
}

export function captureOpenShizuku(): Promise<void> {
  if (hasWebBridge()) return Promise.resolve();
  return call(UI_COMMANDS.capture_open_shizuku);
}

export function captureSetupInstructions(): Promise<CaptureSetupInstructions> {
  return call(UI_COMMANDS.capture_setup_instructions);
}

export function captureOpenDeveloperOptions(): Promise<void> {
  if (hasWebBridge()) return Promise.resolve();
  return call(UI_COMMANDS.capture_open_developer_options);
}

export function captureRequestBatteryExemption(): Promise<void> {
  if (hasWebBridge()) return Promise.resolve();
  return call(UI_COMMANDS.capture_request_battery_exemption);
}
