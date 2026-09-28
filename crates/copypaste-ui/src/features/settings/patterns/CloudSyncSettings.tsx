import { useEffect, useId, useState } from "react";
import { Badge, Button } from "@/components/ui";
import { StateView } from "@/components/shared/StateView";
import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
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
  const accountDescriptionId = useId();
  const [setupRevealKey, setSetupRevealKey] = useState<number>();
  const controller = useCloudAccountController();
  const { cloud, status } = controller;
  const configured = Boolean(status?.configured);
  const connected = Boolean(status?.signed_in && status.key_ready);
  useEffect(() => {
    if (revealAdvancedKey === undefined) return;
    setSetupRevealKey(undefined);
    if (revealAdvancedKey.startsWith("settings.sync.cloud.endpoint.url:") || revealAdvancedKey.startsWith("settings.sync.cloud.endpoint.publishableKey:")) controller.openEndpointEditor();
  }, [revealAdvancedKey, controller.openEndpointEditor]);
  const presentation = cloudSettingsPresentation(status, cloud.isError, cloud.isLoading, controller.syncError);
  const connectionMessage = controller.syncError ? t("settings.sync.cloud.syncError")
    : controller.signOutError ? t("settings.sync.cloud.signOutError")
    : status?.last_error ? t("settings.sync.cloud.lastError")
    : status?.unreadable_uploads ? t("settings.sync.cloud.unreadableUploads", { count: status.unreadable_uploads }) : null;
  const statusControl = presentation.state === "checking" ? <StateView mode="loading" placement="control" />
    : presentation.state === "unavailable" ? <Button variant="secondary" size="sm" aria-describedby={connectionDescriptionId} onClick={() => void cloud.refetch()}>{t("settings.sync.cloud.retry")}</Button>
    : presentation.badge ? <Badge variant={presentation.badge.variant}>{t(presentation.badge.label)}</Badge> : null;
  // The alert owner stays mounted while cloud status changes, so screen readers
  // receive subsequent sync and sign-out errors through the same live region.
  const connectionNote = <><span id={connectionDescriptionId}>{cloud.isLoading ? <StateView mode="loading" placement="control" title={t(presentation.description)} /> : t(presentation.description)}</span><span className={styles.connectionNote} role="alert" aria-live="assertive" aria-atomic="true">{connectionMessage ? <StateView mode="error" placement="control" role="presentation" title={connectionMessage} /> : null}</span></>;
  return <div className={styles.root}><SettingsSchemaRenderer groups={[
    { id: "cloud-connection", title: t("settings.sync.cloud.sectionTitle"), description: t("settings.sync.cloud.sectionDescription"), fields: [
      { kind: "status", definition: settingDefinition("cloud-sync", "settings.sync.cloud.connectionTitle"), value: statusControl, note: connectionNote, help: undefined },
      { kind: "action", definition: settingDefinition("cloud-sync", "settings.sync.cloud.setupTitle"), visible: !cloud.isLoading && !cloud.isError && !configured, label: t("settings.sync.cloud.setupAction"), note: t("settings.sync.cloud.setupDescription"), onAction: () => setSetupRevealKey((key) => (key ?? 0) + 1) },
      { kind: "custom", definition: settingDefinition("cloud-sync", "settings.sync.cloud.accountTitle"), rowless: true, visible: !cloud.isLoading && !cloud.isError && configured,
        content: <section className={styles.setupSection} aria-labelledby="cloud-account-title"><div className={styles.sectionHeader}><div><h4 id="cloud-account-title">{t("settings.sync.cloud.accountTitle")}</h4><p id={accountDescriptionId}>{t(connected ? "settings.sync.cloud.accountConnectedDescription" : "settings.sync.cloud.accountSignedOutDescription")}</p></div></div>{connected ? <CloudConnectedControls controller={controller} descriptionId={accountDescriptionId} /> : <CloudAccountForm controller={controller} />}</section>,
      },
    ] },
    { id: "cloud-server", title: t("settings.sync.cloud.endpoint.advancedTitle"), description: controller.endpointDirty ? t("settings.sync.cloud.endpoint.unsaved") : t("settings.sync.cloud.endpoint.advancedDescription"), disclosure: "cloud-server", revealKey: setupRevealKey === undefined ? revealAdvancedKey : `setup:${setupRevealKey}`, fields: [
      { kind: "action", definition: settingDefinition("cloud-sync", "settings.sync.cloud.endpoint.title"), visible: !cloud.isLoading && !cloud.isError && configured && !controller.endpointEditorOpen, disabled: controller.busy, label: t("settings.sync.cloud.endpoint.change"), icon: "settings", onAction: controller.openEndpointEditor, help: t("settings.sync.cloud.endpoint.configuredDescription") },
      { kind: "custom", definition: settingDefinition("cloud-sync", "settings.sync.cloud.endpoint.url"), rowless: true, visible: !cloud.isLoading && !cloud.isError && (!configured || controller.endpointEditorOpen), content: <CloudEndpointForm controller={controller} replacing={configured} /> },
    ] },
  ]} /></div>;
}
