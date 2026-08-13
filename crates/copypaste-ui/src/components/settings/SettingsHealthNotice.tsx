/**
 * Says so when the service is not running on the settings the user chose.
 *
 * Without this the failure is silent and looks like a preference: private mode
 * comes back on, sync and network visibility come back off, and the exclusion
 * list looks empty. A user who never told the app to do any of that has no way
 * to tell a fail-closed fallback from a bug — or, if the fallback were ever
 * removed, from their privacy settings quietly turning themselves off.
 */
import { ShieldAlert } from "lucide-react";

import { StateNotice } from "@/components/StateNotice";
import { useStatus } from "@/hooks/useHistory";
import { useTranslation } from "@/i18n";
import type { SettingsHealth, StatusData } from "@/generated/ipc";

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
    <StateNotice
      alert
      icon={ShieldAlert}
      tone="attention"
      title={t("settings.service.degraded.title")}
      body={[
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
    />
  );
}
