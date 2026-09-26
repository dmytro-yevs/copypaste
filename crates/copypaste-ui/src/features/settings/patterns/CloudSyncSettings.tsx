import { useEffect, useId, useState } from "react";

import { Badge, Button, Icon } from "@/components/ui";
import { FieldFeedback, SkeletonText } from "@/components/shared";
import { Section } from "@/features/settings/components/Section";
import { SettingsDisclosure } from "@/features/settings/components/SettingsDisclosure";
import { useCloudAccountController } from "@/features/settings/hooks/useCloudAccountController";
import { cloudSettingsPresentation } from "@/features/settings/model/cloudPresentation";
import { CloudAccountForm } from "@/features/settings/patterns/cloud/CloudAccountForm";
import { CloudConnectedControls } from "@/features/settings/patterns/cloud/CloudConnectedControls";
import { CloudEndpointForm } from "@/features/settings/patterns/cloud/CloudEndpointForm";
import { useTranslation } from "@/i18n";
import styles from "./CloudSyncSettings.module.css";

export function CloudSyncSettings({ revealAdvancedKey }: { revealAdvancedKey?: string }) {
  const { t } = useTranslation();
  const connectionDescriptionId = useId();
  const setupDescriptionId = useId();
  const serverDescriptionId = useId();
  const accountDescriptionId = useId();
  const [setupRevealKey, setSetupRevealKey] = useState<number>();
  const controller = useCloudAccountController();
  const { cloud, status } = controller;
  const configured = Boolean(status?.configured);
  const connected = Boolean(status?.signed_in && status.key_ready);

  useEffect(() => {
    if (revealAdvancedKey === undefined) return;
    setSetupRevealKey(undefined);
    if (
      revealAdvancedKey.startsWith("settings.sync.cloud.endpoint.url:") ||
      revealAdvancedKey.startsWith("settings.sync.cloud.endpoint.publishableKey:")
    ) controller.openEndpointEditor();
  }, [revealAdvancedKey, controller.openEndpointEditor]);
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
    <div className={styles.root}>
      <Section
        title={t("settings.sync.cloud.sectionTitle")}
        description={t("settings.sync.cloud.sectionDescription")}
      >
        <div className={styles.setup}>
          <header
            className={styles.setupHeader}
            data-settings-search-target={`row:${t("settings.sync.cloud.connectionTitle")}`}
          >
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

          {!cloud.isLoading && !cloud.isError && !configured ? (
            <section
              className={styles.setupSection}
              aria-labelledby="cloud-setup-title"
            >
              <div className={styles.sectionHeader}>
                <div>
                  <h4 id="cloud-setup-title">{t("settings.sync.cloud.setupTitle")}</h4>
                  <p id={setupDescriptionId}>{t("settings.sync.cloud.setupDescription")}</p>
                </div>
              </div>
              <Button
                variant="secondary"
                size="sm"
                className={styles.setupAction}
                aria-describedby={setupDescriptionId}
                onClick={() => setSetupRevealKey((key) => (key ?? 0) + 1)}
              >
                {t("settings.sync.cloud.setupAction")}
              </Button>
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
      {!cloud.isLoading && !cloud.isError ? (
        <SettingsDisclosure
          title={t("settings.sync.cloud.endpoint.advancedTitle")}
          description={controller.endpointDirty
            ? t("settings.sync.cloud.endpoint.unsaved")
            : t("settings.sync.cloud.endpoint.advancedDescription")}
          revealKey={setupRevealKey === undefined ? revealAdvancedKey : `setup:${setupRevealKey}`}
        >
          <section
            className={styles.endpointPanel}
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
        </SettingsDisclosure>
      ) : null}
    </div>
  );
}
