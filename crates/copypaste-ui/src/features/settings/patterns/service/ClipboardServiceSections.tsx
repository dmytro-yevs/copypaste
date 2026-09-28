import { SourceExclusions } from "@/features/capture";
import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import {
  DEDUP_WINDOW_SECS, MAX_DECODED_IMAGE_MB, MAX_FILE_SIZE_BYTES,
  MAX_FILE_SIZE_BYTES_LIMIT, MAX_IMAGE_SIZE_BYTES, MAX_TEXT_SIZE_BYTES,
  MIN_DECODED_IMAGE_MB, MIN_FILE_SIZE_BYTES, MIN_IMAGE_SIZE_BYTES,
  MIN_TEXT_SIZE_BYTES, POLL_INTERVAL_MAX_MS, POLL_INTERVAL_MIN_MS,
  POLL_INTERVAL_MS, valuesWith, type Choice,
} from "@/features/settings/model/serviceChoices";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import type { SettingsField } from "@/features/settings/model/settingsFieldSchema";
import { useTranslation } from "@/i18n";
import styles from "../ServiceTab.module.css";
import { ServiceFieldNote } from "./ServiceFieldNote";
import { useServiceSettings } from "./ServiceSettingsController";

const compactNumber = new Intl.NumberFormat(undefined, { notation: "compact", maximumFractionDigits: 0 });

export function ClipboardCaptureSection({ revealAdvancedKey }: { revealAdvancedKey?: string }) {
  const { t } = useTranslation();
  const controller = useServiceSettings();
  const { data } = controller;
  type NumericField = "dedup_window_secs" | "poll_interval_ms" | "max_text_size_bytes" | "max_image_size_bytes" | "max_file_size_bytes" | "max_decoded_image_mb";
  const choice = (key: NumericField, title: string, choices: readonly Choice[], icon: "copy" | "refresh" | "fileText" | "fileImage" | "file", validation?: { min: number; max?: number; message: string }): SettingsField => ({
    kind: "choice",
    definition: settingDefinition("clipboard", title),
    value: String(data[key]),
    options: valuesWith(choices, data[key]).map((item) => ({
      value: String(item.value),
      label: item.unit === "items" ? compactNumber.format(item.count) : t(`settings.service.units.${item.unit}`, { count: item.count }),
    })),
    leadingIcon: icon,
    disabled: controller.fieldPending(key),
    busy: controller.fieldPending(key),
    note: <ServiceFieldNote field={key} />,
    validation,
    onChange: (value) => controller.apply({ [key]: Number(value) }),
  });
  return <SettingsSchemaRenderer groups={[
    { id: "capture", title: t("settings.service.groups.capture.title"), description: t("settings.service.groups.capture.description"), fields: [
      choice("dedup_window_secs", "settings.service.dedup.title", DEDUP_WINDOW_SECS, "copy"),
      { kind: "custom", definition: settingDefinition("clipboard", "settings.service.exclusions.title"), rowless: true, busy: controller.fieldPending("excluded_app_bundle_ids"), content: <div className={styles.embeddedExclusions}><SourceExclusions ids={data.excluded_app_bundle_ids} disabled={controller.fieldPending("excluded_app_bundle_ids")} onChange={(excluded_app_bundle_ids) => controller.apply({ excluded_app_bundle_ids })} />{(controller.fieldPending("excluded_app_bundle_ids") || controller.fieldFailed("excluded_app_bundle_ids")) ? <div className={styles.exclusionsFeedback}><ServiceFieldNote field="excluded_app_bundle_ids" /></div> : null}</div> },
    ] },
    { id: "clipboard-advanced", title: t("settings.service.advanced.title"), description: t("settings.service.advanced.description"), disclosure: "clipboard-advanced", revealKey: revealAdvancedKey, fields: [
      choice("poll_interval_ms", "settings.service.poll.title", POLL_INTERVAL_MS, "refresh", { min: POLL_INTERVAL_MIN_MS, max: POLL_INTERVAL_MAX_MS, message: t("settings.service.validation.poll") }),
      choice("max_text_size_bytes", "settings.service.maxText.title", MAX_TEXT_SIZE_BYTES, "fileText", { min: MIN_TEXT_SIZE_BYTES, max: MAX_FILE_SIZE_BYTES_LIMIT, message: t("settings.service.validation.text") }),
      choice("max_image_size_bytes", "settings.service.maxImage.title", MAX_IMAGE_SIZE_BYTES, "fileImage", { min: MIN_IMAGE_SIZE_BYTES, max: MAX_FILE_SIZE_BYTES_LIMIT, message: t("settings.service.validation.image") }),
      choice("max_file_size_bytes", "settings.service.maxFile.title", MAX_FILE_SIZE_BYTES, "file", { min: MIN_FILE_SIZE_BYTES, max: MAX_FILE_SIZE_BYTES_LIMIT, message: t("settings.service.validation.file") }),
      choice("max_decoded_image_mb", "settings.service.maxDecodedImage.title", MAX_DECODED_IMAGE_MB, "fileImage", { min: MIN_DECODED_IMAGE_MB, message: t("settings.service.validation.decodedImage") }),
    ] },
  ]} />;
}

export function ClipboardNotificationSection({ supportsCopyNotifications }: { supportsCopyNotifications: boolean }) {
  const { t } = useTranslation();
  const controller = useServiceSettings();
  const { data } = controller;
  return <SettingsSchemaRenderer groups={[{
    id: "notifications", title: t("settings.service.groups.telling.title"), fields: [
      { kind: "boolean", definition: settingDefinition("clipboard", "settings.service.notify.title"), value: data.notify_on_copy, controlId: "notify-on-copy", visible: supportsCopyNotifications, disabled: controller.fieldPending("notify_on_copy"), busy: controller.fieldPending("notify_on_copy"), note: <ServiceFieldNote field="notify_on_copy" />, onChange: (notify_on_copy) => controller.apply({ notify_on_copy }) },
      { kind: "boolean", definition: settingDefinition("clipboard", "settings.service.sound.title"), value: data.sound_on_copy, controlId: "sound-on-copy", disabled: controller.fieldPending("sound_on_copy"), busy: controller.fieldPending("sound_on_copy"), note: <ServiceFieldNote field="sound_on_copy" />, onChange: (sound_on_copy) => controller.apply({ sound_on_copy }) },
    ],
  }]} />;
}
