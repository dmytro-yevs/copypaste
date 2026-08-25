import { Section } from "@/features/settings/components/Section";
import { SwitchRow } from "@/features/settings/components/SwitchRow";
import { useTranslation } from "@/i18n";
import { ServiceFieldNote } from "./ServiceFieldNote";
import { useServiceSettings } from "./ServiceSettingsController";

export function AdvancedServiceSection() {
  const { t } = useTranslation();
  const controller = useServiceSettings();
  const { data } = controller;

  return (
    <Section title={t("settings.service.groups.network.title")}>
      <SwitchRow
        title={t("settings.service.syncEnabled.title")}
        description={t("settings.service.syncEnabled.description")}
        id="sync-enabled"
        checked={data.sync_enabled}
        disabled={controller.fieldPending("sync_enabled")}
        busy={controller.fieldPending("sync_enabled")}
        note={<ServiceFieldNote field="sync_enabled" />}
        onChange={(sync_enabled) => controller.apply({ sync_enabled })}
      />

      <SwitchRow
        title={t("settings.service.lan.title")}
        description={t("settings.service.lan.description")}
        id="lan-visibility"
        checked={data.lan_visibility}
        disabled={controller.fieldPending("lan_visibility")}
        busy={controller.fieldPending("lan_visibility")}
        note={<ServiceFieldNote field="lan_visibility" />}
        onChange={(lan_visibility) => controller.apply({ lan_visibility })}
      />
    </Section>
  );
}
