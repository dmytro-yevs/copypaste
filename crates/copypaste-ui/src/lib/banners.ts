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

export interface Banner {
  readonly id: "history-unreadable";
  readonly message: string;
}

export interface BannerConditions {
  /** In the shell rather than on History alone: with no readable history,
   *  Devices and Settings fail too, and a user standing on either would see
   *  three unexplained failures instead of one explained one. */
  historyUnreadable: "key_unusable" | null;
}

/** This state has no recovery action and cannot be dismissed. */
export function pickBanner(conditions: BannerConditions): Banner | null {
  if (conditions.historyUnreadable !== null) {
    return {
      id: "history-unreadable",
      message: t("shell.banner.keyUnusable"),
    };
  }
  return null;
}
