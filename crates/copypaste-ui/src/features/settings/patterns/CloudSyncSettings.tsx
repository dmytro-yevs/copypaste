import { useId } from "react";

import { Badge, Button, Icon } from "@/components/ui";
import { FieldFeedback, SkeletonText } from "@/components/shared";
import { Section } from "@/features/settings/components/Section";
import { useCloudAccountController } from "@/features/settings/hooks/useCloudAccountController";
import { cloudSettingsPresentation } from "@/features/settings/model/cloudPresentation";
import { CloudAccountForm } from "@/features/settings/patterns/cloud/CloudAccountForm";
import { CloudConnectedControls } from "@/features/settings/patterns/cloud/CloudConnectedControls";
import { CloudEndpointForm } from "@/features/settings/patterns/cloud/CloudEndpointForm";
import { useTranslation } from "@/i18n";
import styles from "./CloudSyncSettings.module.css";

export function CloudSyncSettings() {
  const { t } = useTranslation();
  const connectionDescriptionId = useId();
  const serverDescriptionId = useId();
  const accountDescriptionId = useId();
  const controller = useCloudAccountController();
  const { cloud, status } = controller;
  const configured = Boolean(status?.configured);
  const connected = Boolean(status?.signed_in && status.key_ready);
  const cloudPresentation = cloudSettingsPresentation(
    status,
    cloud.isError,
    cloud.isLoading,
    controller.syncError,
  );

  const connectionDescription = t(cloudPresentation.description);

  const connectionMessage = controller.syncError
    ? t("settings.sync.cloud.syncError")
    : controller.signOutError
      ? t("settings.sync.cloud.signOutError")
      : status?.last_error
        ? t("settings.sync.cloud.lastError")
        : status?.unreadable_uploads
          ? t("settings.sync.cloud.unreadableUploads", {
              count: status.unreadable_uploads,
            })
          : null;

  const statusControl = cloudPresentation.state === "checking" ? (
    <SkeletonText width="xs" />
  ) : cloudPresentation.state === "unavailable" ? (
    <Button
      variant="secondary"
      size="sm"
      aria-describedby={connectionDescriptionId}
      onClick={() => void cloud.refetch()}
    >
      {t("settings.sync.cloud.retry")}
    </Button>
  ) : cloudPresentation.badge ? (
    <Badge variant={cloudPresentation.badge.variant}>
      {t(cloudPresentation.badge.label)}
    </Badge>
  ) : null;
  const statusIcon = cloudPresentation.icon;

  return (
    <Section
      title={t("settings.sync.cloud.sectionTitle")}
      description={t("settings.sync.cloud.sectionDescription")}
    >
      <div
        className={styles.setup}
        data-settings-search-target={`row:${t("settings.sync.cloud.connectionTitle")}`}
      >
        <header className={styles.setupHeader}>
          <span className={styles.setupIcon} aria-hidden="true">
            <Icon name={statusIcon} size="md" />
          </span>
          <div className={styles.setupCopy}>
            <h3>{t("settings.sync.cloud.connectionTitle")}</h3>
            {cloud.isLoading ? (
              <SkeletonText width="md" />
            ) : (
              <p id={connectionDescriptionId}>{connectionDescription}</p>
            )}
            <span
              className={styles.connectionNote}
              role="alert"
              aria-live="assertive"
              aria-atomic="true"
            >
              {connectionMessage ? (
                <FieldFeedback state="error" announce={false}>
                  {connectionMessage}
                </FieldFeedback>
              ) : null}
            </span>
          </div>
          <div className={styles.setupStatus}>{statusControl}</div>
        </header>

        {!cloud.isLoading && !cloud.isError ? (
          <section
            className={styles.setupSection}
            aria-labelledby="cloud-server-title"
            data-settings-search-target={`row:${t("settings.sync.cloud.endpoint.title")}`}
          >
            <div className={styles.sectionHeader}>
              <div>
                <h4 id="cloud-server-title">{t("settings.sync.cloud.endpoint.title")}</h4>
                <p id={serverDescriptionId}>{t(configured
                  ? "settings.sync.cloud.endpoint.configuredDescription"
                  : "settings.sync.cloud.endpoint.description")}</p>
              </div>
              {configured && !controller.endpointEditorOpen ? (
                <Button
                  variant="secondary"
                  size="sm"
                  disabled={controller.busy}
                  aria-describedby={serverDescriptionId}
                  onClick={controller.openEndpointEditor}
                >
                  <Icon name="settings" aria-hidden="true" />
                  {t("settings.sync.cloud.endpoint.change")}
                </Button>
              ) : null}
            </div>
            {!configured || controller.endpointEditorOpen ? (
              <CloudEndpointForm controller={controller} replacing={configured} />
            ) : null}
          </section>
        ) : null}

        {!cloud.isLoading && !cloud.isError && configured ? (
          <section
            className={styles.setupSection}
            aria-labelledby="cloud-account-title"
            data-settings-search-target={`row:${t("settings.sync.cloud.accountTitle")}`}
          >
            <div className={styles.sectionHeader}>
              <div>
                <h4 id="cloud-account-title">{t("settings.sync.cloud.accountTitle")}</h4>
                <p id={accountDescriptionId}>{t(connected
                  ? "settings.sync.cloud.accountConnectedDescription"
                  : "settings.sync.cloud.accountSignedOutDescription")}</p>
              </div>
            </div>
            {connected ? (
              <CloudConnectedControls
                controller={controller}
                descriptionId={accountDescriptionId}
              />
            ) : (
              <CloudAccountForm controller={controller} />
            )}
          </section>
        ) : null}
      </div>
    </Section>
  );
}
