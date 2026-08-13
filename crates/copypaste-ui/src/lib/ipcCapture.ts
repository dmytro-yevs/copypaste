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
import type { ReadonlyDeep } from "type-fest";
import type { Item } from "./ipc";
import { call, type IpcCallOptions } from "./ipcCall";

export type CapturedPayload = ReadonlyDeep<GeneratedCapturedPayload>;
export type CaptureHealth = ReadonlyDeep<GeneratedCaptureHealth>;
export type CaptureNextStep = GeneratedCaptureNextStep;
export type CaptureRung = GeneratedCaptureRung;
export type CaptureSnapshot = ReadonlyDeep<GeneratedCaptureSnapshot>;
export type CaptureSource = GeneratedCaptureSource;
export type NotGrantedReason = GeneratedNotGrantedReason;
export type NotWorkingReason = GeneratedNotWorkingReason;
export type ShizukuProbe = ReadonlyDeep<GeneratedShizukuProbe>;

export function captureState(options?: IpcCallOptions): Promise<CaptureSnapshot> {
  return call<CaptureSnapshot>("capture_state", undefined, options);
}

/** Called on every resume, not only at startup: a grant can lapse while the app
 *  is backgrounded, and a reboot is the ordinary case. */
export function captureRefresh(): Promise<CaptureSnapshot> {
  return call<CaptureSnapshot>("capture_refresh");
}

/** One call for two steps: it asks for the permission when that is what is
 *  missing, and registers the listener when it is not. */
export function captureArm(): Promise<CaptureSnapshot> {
  return call<CaptureSnapshot>("capture_arm");
}

export function captureDisarm(): Promise<CaptureSnapshot> {
  return call<CaptureSnapshot>("capture_disarm");
}

export function captureSetEnabled(enabled: boolean): Promise<CaptureSnapshot> {
  return call<CaptureSnapshot>("capture_set_enabled", { enabled });
}

/** Rung 0: save whatever is on the clipboard right now. `null` means there was
 *  nothing to save, which is not a failure. */
export function captureNow(source: CaptureSource): Promise<Item | null> {
  return call<Item | null>("capture_now", { source });
}

/** The exact text `authorise_toast` gates on. Fetched rather than copied into
 *  the catalogue: a second copy is a second thing to keep true. */
export function captureToastExplanation(): Promise<string> {
  return call<string>("capture_toast_explanation");
}

/** `acknowledged` may only be `true` when the user has read
 *  `captureToastExplanation` and agreed to it — passing `true` without having
 *  shown the text lies to a gate Rust enforces. Turning suppression **off** is
 *  never gated. */
export function captureSetToastSuppressed(
  suppressed: boolean,
  acknowledged: boolean,
): Promise<CaptureSnapshot> {
  return call<CaptureSnapshot>("capture_set_toast_suppressed", {
    suppressed,
    acknowledged,
  });
}
