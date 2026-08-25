import { InlineNotice } from "@/components/shared";
import type { SettingsHealth, StatusData } from "@/generated/ipc";
import { useStatus } from "@/hooks/useStatus";
import { useTranslation } from "@/i18n";

/** Module scope: `useStatus` needs a stable selector to keep structural
 *  sharing, or `counters.uptime_secs` re-renders this twice a second. */
const selectHealth = (data: StatusData): SettingsHealth | null =>
  data.settings_health;

/** Written out rather than inferred as `string`, so a renamed catalogue entry
 *  is a compile error here instead of a raw key rendered to the user. */
type PrivacyLabel =
  | "settings.service.privateMode.title"
  | "settings.service.exclusions.title"
  | "settings.service.lan.title"
  | "settings.service.syncEnabled.title";

/** The fields whose fallback is a privacy decision rather than a default. */
const PRIVACY_LABELS: Record<string, PrivacyLabel> = {
  private_mode: "settings.service.privateMode.title",
  excluded_app_bundle_ids: "settings.service.exclusions.title",
  lan_visibility: "settings.service.lan.title",
  sync_enabled: "settings.service.syncEnabled.title",
};

export function SettingsHealthNotice() {
  const { t } = useTranslation();
  const { data: health } = useStatus(selectHealth);

  if (!health) return null;
  const { record_unreadable, unreadable_fields } = health;
  if (!record_unreadable && unreadable_fields.length === 0) return null;

  const affected = record_unreadable
    ? Object.values(PRIVACY_LABELS)
    : unreadable_fields
        .map((field) => PRIVACY_LABELS[field])
        .filter((label): label is PrivacyLabel => label !== undefined);
  const others = record_unreadable
    ? 0
    : unreadable_fields.length - affected.length;

  return (
    <InlineNotice live icon="alert" tone="warning">
      <strong>{t("settings.service.degraded.title")}</strong>{" "}
      {[
        affected.length > 0
          ? t("settings.service.degraded.privacy", {
              fields: affected.map((label) => t(label)).join(", "),
            })
          : undefined,
        others > 0
          ? t("settings.service.degraded.others", { count: others })
          : undefined,
        t("settings.service.degraded.remedy"),
      ]
        .filter(Boolean)
        .join(" ")}
    </InlineNotice>
  );
}
