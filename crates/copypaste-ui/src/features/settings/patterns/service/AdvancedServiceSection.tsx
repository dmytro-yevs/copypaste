import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import { useTranslation } from "@/i18n";
import { ServiceFieldNote } from "./ServiceFieldNote";
import { useServiceSettings } from "./ServiceSettingsController";

export function AdvancedServiceSection() {
  const { t } = useTranslation();
  const controller = useServiceSettings();
  const { data } = controller;
  return <SettingsSchemaRenderer groups={[{
    id: "network", title: t("settings.service.groups.network.title"), fields: [
      { kind: "boolean", definition: settingDefinition("device-sync", "settings.service.syncEnabled.title"), value: data.sync_enabled, controlId: "sync-enabled", disabled: controller.fieldPending("sync_enabled"), busy: controller.fieldPending("sync_enabled"), note: <ServiceFieldNote field="sync_enabled" />, onChange: (sync_enabled) => controller.apply({ sync_enabled }) },
      { kind: "boolean", definition: settingDefinition("device-sync", "settings.service.lan.title"), value: data.lan_visibility, controlId: "lan-visibility", disabled: controller.fieldPending("lan_visibility"), busy: controller.fieldPending("lan_visibility"), note: <ServiceFieldNote field="lan_visibility" />, onChange: (lan_visibility) => controller.apply({ lan_visibility }) },
    ],
  }]} />;
}
