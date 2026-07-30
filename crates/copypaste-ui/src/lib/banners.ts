/**
 * INV-17 — **only one alert banner is visible at a time.**
 *
 * v1 had four independent `role="alert"` mechanisms and a ternary chain to
 * de-conflict them; when the chain was wrong they stacked (CopyPaste-8ebg.39).
 * Manifest §9.2 says model them as one ordered list with a severity instead,
 * which is this file: a pure function from conditions to at most one banner.
 * Losers are not discarded — the next call returns the highest-priority
 * condition that is still true, so a banner appears the moment the one above it
 * clears, without needing to be re-triggered (AT-25).
 *
 * No banner text is ever derived from an error object (INV-12).
 */

export type BannerSeverity = "error" | "warning" | "info";

export interface Banner {
  readonly id: "service-offline" | "protocol-mismatch" | "capture-paused";
  readonly severity: BannerSeverity;
  readonly message: string;
  /** P0 conditions cannot be dismissed: the user cannot act on the app at all
   *  while they hold. */
  readonly dismissible: boolean;
  /** A single recovery action, per the manifest's §3.4.2 rule that a banner
   *  offers one way out. */
  readonly action?: "retry";
}

export interface BannerConditions {
  serviceOffline: boolean;
  /** The version the daemon reports, when it differs from ours. */
  protocolMismatch: number | null;
  capturePaused: boolean;
  dismissed: readonly string[];
}

/** Ordered by priority. The first match whose id is not dismissed wins. */
export function pickBanner(conditions: BannerConditions): Banner | null {
  const candidates: Banner[] = [];

  if (conditions.serviceOffline) {
    candidates.push({
      id: "service-offline",
      severity: "error",
      message: "Background service not running — nothing is being recorded.",
      dismissible: false,
    });
  }

  if (conditions.protocolMismatch !== null) {
    candidates.push({
      id: "protocol-mismatch",
      severity: "warning",
      message: `CopyPaste and the background service are on incompatible versions (service protocol v${conditions.protocolMismatch}). Restart both to resolve.`,
      dismissible: true,
    });
  }

  if (conditions.capturePaused) {
    candidates.push({
      id: "capture-paused",
      severity: "warning",
      message:
        "The clipboard service is running but is not recording — copied items will not appear here.",
      dismissible: true,
      action: "retry",
    });
  }

  for (const candidate of candidates) {
    if (candidate.dismissible && conditions.dismissed.includes(candidate.id)) {
      continue;
    }
    return candidate;
  }
  return null;
}
