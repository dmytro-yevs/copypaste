/**
 * No sentence is derived here: `headline` and `detail` arrive finished from
 * `capture::messages`, and a second wording keyed off `health` is the drift
 * that split v1's setup screen from its notification.
 */
import type { CaptureHealth, CaptureNextStep, CaptureSnapshot } from "@/lib/ipc";

/** Presentation weight, not severity. The states a reboot produces are
 *  `attention`, not `fault`: Shizuku stops on every restart and re-arming is
 *  routine. `fault` is reserved for a read that was refused. */
export type CaptureTone = "ok" | "info" | "attention" | "fault" | "off";

export function toneOf(health: CaptureHealth): CaptureTone {
  switch (health.state) {
    case "working":
      return "ok";
    case "disabled":
      return "off";
    case "not_granted":
      return health.reason === "unsupported" || health.reason === "not_installed"
        ? "info"
        : "attention";
    case "granted_not_working":
      return health.reason === "read_refused"
        ? "fault"
        : health.reason === "awaiting_first_copy"
          ? "info"
          : "attention";
  }
}

/** Tailwind needs whole class names, so the dot is a lookup rather than an
 *  interpolation. It is decorative everywhere it is used: the sentence beside
 *  it carries the state (A11Y-10). */
export const TONE_DOT: Record<CaptureTone, string> = {
  ok: "bg-ok",
  info: "bg-info",
  attention: "bg-warn",
  fault: "bg-err",
  off: "bg-mute",
};

/** Rung 2's four steps. Rungs 1 and 3 are not built and have no state in the
 *  model, so there is nothing here that could imply they exist. */
export const LADDER_STEPS = ["install", "start", "permission", "armed"] as const;
export type LadderStep = (typeof LADDER_STEPS)[number];

const STEP_FOR = {
  install_shizuku: "install",
  start_shizuku: "start",
  grant_permission: "permission",
  arm: "armed",
  none: undefined,
} as const satisfies Record<CaptureNextStep, LadderStep | undefined>;

export interface LadderRung {
  readonly id: LadderStep;
  readonly done: boolean;
  readonly current: boolean;
}

/** Every `done` comes from a field the platform reported. `armed` is the one
 *  derivation: the snapshot carries no `armed` flag, and `not_armed` is the
 *  health the model reports when the listener is not registered. */
export function ladderOf(snapshot: CaptureSnapshot): readonly LadderRung[] {
  const { shizuku, health, nextStep } = snapshot;
  const current = STEP_FOR[nextStep];
  const armed =
    health.state === "working" ||
    (health.state === "granted_not_working" && health.reason !== "not_armed");

  const done: Record<LadderStep, boolean> = {
    install: shizuku.installed,
    start: shizuku.running,
    permission: shizuku.permission,
    armed,
  };

  return LADDER_STEPS.map((id) => ({
    id,
    done: done[id],
    current: id === current,
  }));
}

/** `recheck` is not a downgrade of the step: CopyPaste can neither install
 *  Shizuku nor start it, so a button carrying either name would do nothing. */
export type CapturePrimary = "arm" | "permission" | "recheck" | "none";

export function primaryOf(nextStep: CaptureNextStep): CapturePrimary {
  switch (nextStep) {
    case "arm":
      return "arm";
    case "grant_permission":
      return "permission";
    case "install_shizuku":
    case "start_shizuku":
      return "recheck";
    case "none":
      return "none";
  }
}
