/**
 * No sentence is derived here: `headline` and `detail` arrive finished from
 * `capture::messages`; a second wording keyed off `health` would let setup and
 * notifications drift.
 */
import type { CaptureHealth } from "@/lib/ipc";

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
