import { FieldFeedback } from "@/components/shared";
import { ChoiceRow } from "@/features/settings/components/ChoiceRow";
import { Section } from "@/features/settings/components/Section";
import { SwitchRow } from "@/features/settings/components/SwitchRow";
import {
  HISTORY_LIMIT,
  RETENTION_DAYS,
  SENSITIVE_TTL_SECS,
  STORAGE_QUOTA_BYTES,
} from "@/features/settings/model/serviceChoices";
import { useTranslation } from "@/i18n";
import styles from "../ServiceTab.module.css";
import { ServiceFieldNote } from "./ServiceFieldNote";
import { useServiceSettings } from "./ServiceSettingsController";

export function PrivacyServiceSections() {
  const { t } = useTranslation();
  const controller = useServiceSettings();
  const { data } = controller;
  const sweeping = data.sensitive_ttl_secs > 0;

  return (
    <>
      <Section title="Private mode">
        <SwitchRow
          title={t("settings.service.privateMode.title")}
          description={t("settings.service.privateMode.description")}
          id="private-mode"
          checked={controller.privateModeEnabled ?? false}
          disabled={controller.privateModePending}
          busy={controller.privateModePending}
          note={
            controller.privateModePending ? (
              <FieldFeedback state="pending">
                Saving…
              </FieldFeedback>
            ) : controller.privateModeFailed ? (
              <FieldFeedback state="error">
                Private mode wasn’t changed.
              </FieldFeedback>
            ) : undefined
          }
          onChange={controller.setPrivateMode}
        />
      </Section>

      <Section title={t("settings.service.groups.keeping.title")}>
        <ChoiceRow
          title={t("settings.service.historyLimit.title")}
          description={t("settings.service.historyLimit.description")}
          icon="library"
          choices={HISTORY_LIMIT}
          value={data.history_limit}
          disabled={controller.fieldPending("history_limit")}
          busy={controller.fieldPending("history_limit")}
          note={<ServiceFieldNote field="history_limit" />}
          onChange={(history_limit) => controller.apply({ history_limit })}
        />

        <ChoiceRow
          title={t("settings.service.storageQuota.title")}
          description={t("settings.service.storageQuota.description")}
          icon="folder"
          choices={STORAGE_QUOTA_BYTES}
          value={data.storage_quota_bytes}
          disabled={controller.fieldPending("storage_quota_bytes")}
          busy={controller.fieldPending("storage_quota_bytes")}
          note={<ServiceFieldNote field="storage_quota_bytes" />}
          onChange={(storage_quota_bytes) =>
            controller.apply({ storage_quota_bytes })
          }
        />

        <ChoiceRow
          title={t("settings.service.retention.title")}
          description={t("settings.service.retention.description")}
          icon="refresh"
          choices={RETENTION_DAYS}
          value={data.retention_days}
          disabled={controller.fieldPending("retention_days")}
          busy={controller.fieldPending("retention_days")}
          note={<ServiceFieldNote field="retention_days" />}
          onChange={(retention_days) => controller.apply({ retention_days })}
        />

        <ChoiceRow
          title={t("settings.service.sensitive.title")}
          description={t("settings.service.sensitive.description")}
          icon="alert"
          note={
            <ServiceFieldNote field="sensitive_ttl_secs">
              {sweeping ? (
                <span className={styles.sensitiveWarning}>
                  {`${t("settings.service.sensitive.warning")} ${t("settings.service.sensitive.announced")}`}
                </span>
              ) : null}
            </ServiceFieldNote>
          }
          choices={SENSITIVE_TTL_SECS}
          value={data.sensitive_ttl_secs}
          disabled={controller.fieldPending("sensitive_ttl_secs")}
          busy={controller.fieldPending("sensitive_ttl_secs")}
          onChange={(sensitive_ttl_secs) =>
            controller.apply({ sensitive_ttl_secs })
          }
        />
      </Section>
    </>
  );
}
