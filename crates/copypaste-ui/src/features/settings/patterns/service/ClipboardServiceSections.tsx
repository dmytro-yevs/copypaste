import { SourceExclusions } from "@/features/capture";
import { ChoiceRow } from "@/features/settings/components/ChoiceRow";
import { Section } from "@/features/settings/components/Section";
import { SwitchRow } from "@/features/settings/components/SwitchRow";
import {
  DEDUP_WINDOW_SECS,
  MAX_DECODED_IMAGE_MB,
  MAX_FILE_SIZE_BYTES,
  MAX_FILE_SIZE_BYTES_LIMIT,
  MAX_IMAGE_SIZE_BYTES,
  MAX_TEXT_SIZE_BYTES,
  MIN_DECODED_IMAGE_MB,
  MIN_FILE_SIZE_BYTES,
  MIN_IMAGE_SIZE_BYTES,
  MIN_TEXT_SIZE_BYTES,
  POLL_INTERVAL_MAX_MS,
  POLL_INTERVAL_MIN_MS,
  POLL_INTERVAL_MS,
} from "@/features/settings/model/serviceChoices";
import { useTranslation } from "@/i18n";
import styles from "../ServiceTab.module.css";
import { ServiceFieldNote } from "./ServiceFieldNote";
import { useServiceSettings } from "./ServiceSettingsController";

export function ClipboardCaptureSection() {
  const { t } = useTranslation();
  const controller = useServiceSettings();
  const { data } = controller;

  return (
    <Section
      title={t("settings.service.groups.capture.title")}
      description={t("settings.service.groups.capture.description")}
    >
      <ChoiceRow
        title={t("settings.service.poll.title")}
        description={t("settings.service.poll.description")}
        icon="refresh"
        choices={POLL_INTERVAL_MS}
        value={data.poll_interval_ms}
        disabled={controller.fieldPending("poll_interval_ms")}
        busy={controller.fieldPending("poll_interval_ms")}
        note={<ServiceFieldNote field="poll_interval_ms" />}
        validation={{
          min: POLL_INTERVAL_MIN_MS,
          max: POLL_INTERVAL_MAX_MS,
          message: t("settings.service.validation.poll"),
        }}
        onChange={(poll_interval_ms) => controller.apply({ poll_interval_ms })}
      />

      <ChoiceRow
        title={t("settings.service.dedup.title")}
        description={t("settings.service.dedup.description")}
        icon="copy"
        choices={DEDUP_WINDOW_SECS}
        value={data.dedup_window_secs}
        disabled={controller.fieldPending("dedup_window_secs")}
        busy={controller.fieldPending("dedup_window_secs")}
        note={<ServiceFieldNote field="dedup_window_secs" />}
        onChange={(dedup_window_secs) => controller.apply({ dedup_window_secs })}
      />

      <ChoiceRow
        title={t("settings.service.maxText.title")}
        description={t("settings.service.maxText.description")}
        icon="fileText"
        choices={MAX_TEXT_SIZE_BYTES}
        value={data.max_text_size_bytes}
        disabled={controller.fieldPending("max_text_size_bytes")}
        busy={controller.fieldPending("max_text_size_bytes")}
        note={<ServiceFieldNote field="max_text_size_bytes" />}
        validation={{
          min: MIN_TEXT_SIZE_BYTES,
          max: MAX_FILE_SIZE_BYTES_LIMIT,
          message: t("settings.service.validation.text"),
        }}
        onChange={(max_text_size_bytes) =>
          controller.apply({ max_text_size_bytes })
        }
      />

      <ChoiceRow
        title={t("settings.service.maxImage.title")}
        description={t("settings.service.maxImage.description")}
        icon="fileImage"
        choices={MAX_IMAGE_SIZE_BYTES}
        value={data.max_image_size_bytes}
        disabled={controller.fieldPending("max_image_size_bytes")}
        busy={controller.fieldPending("max_image_size_bytes")}
        note={<ServiceFieldNote field="max_image_size_bytes" />}
        validation={{
          min: MIN_IMAGE_SIZE_BYTES,
          max: MAX_FILE_SIZE_BYTES_LIMIT,
          message: t("settings.service.validation.image"),
        }}
        onChange={(max_image_size_bytes) =>
          controller.apply({ max_image_size_bytes })
        }
      />

      <ChoiceRow
        title={t("settings.service.maxFile.title")}
        description={t("settings.service.maxFile.description")}
        icon="file"
        choices={MAX_FILE_SIZE_BYTES}
        value={data.max_file_size_bytes}
        disabled={controller.fieldPending("max_file_size_bytes")}
        busy={controller.fieldPending("max_file_size_bytes")}
        note={<ServiceFieldNote field="max_file_size_bytes" />}
        validation={{
          min: MIN_FILE_SIZE_BYTES,
          max: MAX_FILE_SIZE_BYTES_LIMIT,
          message: t("settings.service.validation.file"),
        }}
        onChange={(max_file_size_bytes) =>
          controller.apply({ max_file_size_bytes })
        }
      />

      <ChoiceRow
        title={t("settings.service.maxDecodedImage.title")}
        description={t("settings.service.maxDecodedImage.description")}
        icon="fileImage"
        choices={MAX_DECODED_IMAGE_MB}
        value={data.max_decoded_image_mb}
        disabled={controller.fieldPending("max_decoded_image_mb")}
        busy={controller.fieldPending("max_decoded_image_mb")}
        note={<ServiceFieldNote field="max_decoded_image_mb" />}
        validation={{
          min: MIN_DECODED_IMAGE_MB,
          message: t("settings.service.validation.decodedImage"),
        }}
        onChange={(max_decoded_image_mb) =>
          controller.apply({ max_decoded_image_mb })
        }
      />

      <div
        className={styles.embeddedExclusions}
        aria-busy={
          controller.fieldPending("excluded_app_bundle_ids") || undefined
        }
      >
        <SourceExclusions
          ids={data.excluded_app_bundle_ids}
          disabled={controller.fieldPending("excluded_app_bundle_ids")}
          onChange={(excluded_app_bundle_ids) =>
            controller.apply({ excluded_app_bundle_ids })
          }
        />
        {(controller.fieldPending("excluded_app_bundle_ids") ||
          controller.fieldFailed("excluded_app_bundle_ids")) && (
          <div className={styles.exclusionsFeedback}>
            <ServiceFieldNote field="excluded_app_bundle_ids" />
          </div>
        )}
      </div>
    </Section>
  );
}

export function ClipboardNotificationSection() {
  const { t } = useTranslation();
  const controller = useServiceSettings();
  const { data } = controller;

  return (
    <Section title={t("settings.service.groups.telling.title")}>
      <SwitchRow
        title={t("settings.service.notify.title")}
        description={t("settings.service.notify.description")}
        id="notify-on-copy"
        checked={data.notify_on_copy}
        disabled={controller.fieldPending("notify_on_copy")}
        busy={controller.fieldPending("notify_on_copy")}
        note={<ServiceFieldNote field="notify_on_copy" />}
        onChange={(notify_on_copy) => controller.apply({ notify_on_copy })}
      />

      <SwitchRow
        title={t("settings.service.sound.title")}
        description={t("settings.service.sound.description")}
        id="sound-on-copy"
        checked={data.sound_on_copy}
        disabled={controller.fieldPending("sound_on_copy")}
        busy={controller.fieldPending("sound_on_copy")}
        note={<ServiceFieldNote field="sound_on_copy" />}
        onChange={(sound_on_copy) => controller.apply({ sound_on_copy })}
      />
    </Section>
  );
}
