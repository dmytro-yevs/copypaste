import { StateView } from "@/components/shared/StateView";
import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import { HISTORY_LIMIT, RETENTION_DAYS, STORAGE_QUOTA_BYTES, valuesWith, type Choice } from "@/features/settings/model/serviceChoices";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import type { SettingsField } from "@/features/settings/model/settingsFieldSchema";
import { settingsGroups } from "@/features/settings/model/settingsProjection";
import { useTranslation } from "@/i18n";
import { ServiceFieldNote } from "./ServiceFieldNote";
import { useServiceSettings } from "./ServiceSettingsController";

const compactNumber = new Intl.NumberFormat(undefined, { notation: "compact", maximumFractionDigits: 0 });

export function PrivacyServiceSections() {
  const { t } = useTranslation();
  const controller = useServiceSettings();
  const { data } = controller;
  type Key = "history_limit" | "storage_quota_bytes" | "retention_days";
  const choice = (key: Key, title: string, options: readonly Choice[], icon: "library" | "folder" | "refresh"): SettingsField => ({
    kind: "choice", definition: settingDefinition("privacy", title), value: String(data[key]), leadingIcon: icon,
    options: valuesWith(options, data[key]).map((item) => ({ value: String(item.value), label: item.unit === "items" ? compactNumber.format(item.count) : t(`settings.service.units.${item.unit}`, { count: item.count }) })),
    disabled: controller.fieldPending(key), busy: controller.fieldPending(key),
    note: <ServiceFieldNote field={key} />,
    onChange: (value) => controller.apply({ [key]: Number(value) }),
  });
  const fields: SettingsField[] = [
    {
      kind: "boolean", definition: settingDefinition("privacy", "settings.service.privateMode.title"), value: controller.privateModeEnabled ?? false, controlId: "private-mode", disabled: controller.privateModePending, busy: controller.privateModePending,
      note: controller.privateModePending ? <StateView mode="loading" placement="control" title="Saving…" /> : controller.privateModeFailed ? <StateView mode="error" placement="control" title="Private mode wasn’t changed." /> : undefined,
      onChange: controller.setPrivateMode,
    },
      choice("history_limit", "settings.service.historyLimit.title", HISTORY_LIMIT, "library"),
      choice("storage_quota_bytes", "settings.service.storageQuota.title", STORAGE_QUOTA_BYTES, "folder"),
      choice("retention_days", "settings.service.retention.title", RETENTION_DAYS, "refresh"),
  ];
  return <SettingsSchemaRenderer groups={settingsGroups("privacy", fields, (key) => t(key as never))} />;
}
