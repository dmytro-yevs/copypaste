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
import { t } from "@/i18n";

export type BannerSeverity = "error" | "warning" | "info";

export interface Banner {
  readonly id:
    | "service-offline"
    | "history-unreadable"
    | "protocol-mismatch"
    | "capture-paused"
    | "legacy-history";
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
  /**
   * Which unrecoverable condition the service is stuck in, when it is one.
   *
   * It belongs in the shell rather than only on the History screen because it
   * is not a History problem: with no readable history, Devices and Settings
   * fail too, and a user standing on either of those tabs would otherwise see
   * three unexplained failures instead of one explained one.
   */
  historyUnreadable: "legacy_database" | "key_unusable" | null;
  /** The version the daemon reports, when it differs from ours. */
  protocolMismatch: number | null;
  capturePaused: boolean;
  /**
   * A CopyPaste 0.4 history is on this device and this version has started a
   * new one — `StatusData::legacy_history_present`.
   *
   * Nothing is broken, which is why it is last and why it is dismissible. It
   * is here at all because the alternative is a user seeing an empty history
   * after an upgrade and concluding it was thrown away; the banner is the only
   * surface that reaches them wherever they are looking. CLAUDE.md rule 3's
   * second obligation ends here.
   */
  legacyHistory: boolean;
  dismissed: readonly string[];
}

/**
 * `StatusData::legacy_history_present`, read defensively.
 *
 * The frontend's `StatusData` (`lib/ipc.ts`) does not declare the field — that
 * file is owned by another change in flight — while the bridge passes
 * `copypaste_ipc::StatusData` through verbatim (`model.rs`: `UiStatus =
 * StatusData`), so the value is on the object at runtime. Narrowed once, here,
 * rather than cast at the call site: this is the line to delete when the
 * interface catches up.
 */
export function legacyHistoryPresent(status: unknown): boolean {
  return (
    typeof status === "object" &&
    status !== null &&
    (status as { legacy_history_present?: unknown }).legacy_history_present === true
  );
}

/** Ordered by priority. The first match whose id is not dismissed wins. */
export function pickBanner(conditions: BannerConditions): Banner | null {
  const candidates: Banner[] = [];

  if (conditions.serviceOffline) {
    candidates.push({
      id: "service-offline",
      severity: "error",
      message: t("shell.banner.serviceOffline"),
      dismissible: false,
    });
  }

  if (conditions.historyUnreadable !== null) {
    candidates.push({
      id: "history-unreadable",
      severity: "error",
      message:
        conditions.historyUnreadable === "legacy_database"
          ? t("shell.banner.legacyDatabase")
          : t("shell.banner.keyUnusable"),
      // Not dismissible, for the same reason `service-offline` is not: nothing
      // in the app works while it holds. **And no action** — the banner offers
      // one way out or none, and there is no way out of either of these.
      dismissible: false,
    });
  }

  if (conditions.protocolMismatch !== null) {
    candidates.push({
      id: "protocol-mismatch",
      severity: "warning",
      message: t("shell.banner.protocolMismatch", {
        protocol: conditions.protocolMismatch,
      }),
      dismissible: true,
    });
  }

  if (conditions.capturePaused) {
    candidates.push({
      id: "capture-paused",
      severity: "warning",
      message: t("shell.banner.capturePaused"),
      dismissible: true,
      action: "retry",
    });
  }

  if (conditions.legacyHistory) {
    candidates.push({
      id: "legacy-history",
      severity: "info",
      message: t("shell.banner.legacyHistory"),
      dismissible: true,
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
