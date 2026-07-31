/**
 * INV-17: only one alert banner is visible at a time. v1 de-conflicted four
 * independent `role="alert"` mechanisms with a ternary chain, and when the
 * chain was wrong they stacked (CopyPaste-8ebg.39).
 *
 * Losers are not discarded: the next call returns the highest-priority
 * condition still true, so a banner appears the moment the one above it clears
 * (AT-25). No banner text is ever derived from an error object (INV-12).
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
  /** One way out, or none — never two (manifest §3.4.2). */
  readonly action?: "retry";
}

export interface BannerConditions {
  serviceOffline: boolean;
  /** In the shell rather than on History alone: with no readable history,
   *  Devices and Settings fail too, and a user standing on either would see
   *  three unexplained failures instead of one explained one. */
  historyUnreadable: "legacy_database" | "key_unusable" | null;
  /** The version the daemon reports, when it differs from ours. */
  protocolMismatch: number | null;
  capturePaused: boolean;
  /** `StatusData::legacy_history_present`: a 0.4 history is on this device and
   *  this version has started a new one. Nothing is broken, so it is last and
   *  dismissible; without it an empty history after an upgrade reads as data
   *  thrown away (CLAUDE.md rule 3). */
  legacyHistory: boolean;
  dismissed: readonly string[];
}

/** The frontend's `StatusData` does not declare the field, while the bridge
 *  passes `copypaste_ipc::StatusData` through verbatim, so it is there at
 *  runtime. Delete this when the interface catches up. */
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
      // And no action: there is no way out of either of these, and a banner
      // offers one way out or none.
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
