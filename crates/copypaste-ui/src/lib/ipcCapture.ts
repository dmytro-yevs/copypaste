import type { Item } from "./ipc";
import { call } from "./ipcCall";

/** The mechanism capturing right now — never the one that was asked for. */
export type CaptureRung = "desktop" | "in_app" | "shizuku";

export type NotGrantedReason =
  | "unsupported"
  | "not_installed"
  | "not_running"
  | "no_permission";

export type NotWorkingReason = "awaiting_first_copy" | "read_refused" | "not_armed";

/** `working` is reachable only through a read that happened without focus —
 *  a permission being present is not evidence (CopyPaste-qzhu). Nothing in this
 *  file may construct one. */
export type CaptureHealth =
  | { readonly state: "not_granted"; readonly reason: NotGrantedReason }
  | { readonly state: "disabled" }
  | { readonly state: "granted_not_working"; readonly reason: NotWorkingReason }
  | { readonly state: "working" };

export type CaptureNextStep =
  | "none"
  | "install_shizuku"
  | "start_shizuku"
  | "grant_permission"
  | "arm";

export type CaptureSource = "in_app" | "share" | "process_text" | "tile" | "background";

export interface ShizukuProbe {
  /** Wireless debugging can be paired on the phone itself from Android 11. */
  readonly supported: boolean;
  readonly installed: boolean;
  /** False after every reboot until the user starts it again. */
  readonly running: boolean;
  readonly permission: boolean;
  readonly toastSuppressed: boolean;
  readonly rearmRequested: boolean;
}

/** `headline` and `detail` are finished sentences authored and tested in
 *  `capture::messages` (ADR-0005). Render them verbatim: deriving replacements
 *  from `health` here is what let the setup screen, the status strip and the
 *  loss notification disagree about the state of the device. */
export interface CaptureSnapshot {
  readonly rung: CaptureRung;
  readonly health: CaptureHealth;
  readonly shizuku: ShizukuProbe;
  readonly nextStep: CaptureNextStep;
  readonly headline: string;
  readonly detail: string | null;
  readonly lastReadOkAt: number | null;
  readonly lastCaptureAt: number | null;
  /** Copies that were taken from the platform and never stored. */
  readonly droppedClips: number;
  readonly toastSuppressed: boolean;
  readonly toastAcknowledged: boolean;
  /** The app was opened from the "background capture stopped" notification. */
  readonly rearmRequested: boolean;
}

/** Carries no clipboard content: the list re-reads through `list`. */
export interface CapturedPayload {
  readonly id: string;
  readonly source: CaptureSource;
  readonly isSensitive: boolean;
}

export function captureState(): Promise<CaptureSnapshot> {
  return call<CaptureSnapshot>("capture_state");
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
