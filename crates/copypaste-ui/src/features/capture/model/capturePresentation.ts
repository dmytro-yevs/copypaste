/**
 * No sentence is derived here: `headline` and `detail` arrive finished from
 * `capture::messages`, and a second wording keyed off `health` is the drift
 * that split v1's setup screen from its notification.
 */
import type { CaptureHealth, CaptureNextStep, CaptureSnapshot } from "@/lib/ipc";

/** Setup and restart prompts are `attention`, not `danger`: they are
 *  actionable setup states, not breakage. `danger` is reserved for a read
 *  that was refused. */
export type CaptureTone =
  | "positive"
  | "info"
  | "attention"
  | "danger"
  | "off";
export type CaptureRole = "status" | "alert";
export type CaptureUrgency = "polite" | "assertive";

export interface CapturePresentation {
  readonly tone: CaptureTone;
  readonly role: CaptureRole;
  readonly urgency: CaptureUrgency;
}

export function capturePresentationOf(
  health: CaptureHealth,
): CapturePresentation {
  let tone: CaptureTone;
  switch (health.state) {
    case "working":
      tone = "positive";
      break;
    case "disabled":
      tone = "off";
      break;
    case "not_granted":
      tone = health.reason === "unsupported" || health.reason === "not_installed"
        ? "info"
        : "attention";
      break;
    case "granted_not_working":
      tone = health.reason === "read_refused"
        ? "danger"
        : health.reason === "awaiting_first_copy"
          ? "info"
          : "attention";
      break;
  }

  return tone === "danger"
    ? { tone, role: "alert", urgency: "assertive" }
    : { tone, role: "status", urgency: "polite" };
}

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
 *  health the model reports when the reader is not running. */
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
