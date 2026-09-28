import { useState } from "react";
import { Badge, Button } from "@/components/ui";
import { FieldFeedback } from "@/components/shared";
import { DeviceNameField } from "@/features/devices";
import { connectionSummary, syncReadinessIsLoading, syncReadinessMessage, syncReadinessOf, syncReadinessRecovery } from "@/features/devices/model";
import { noteSync, type PeerHealthMap } from "@/features/devices/model/peerState";
import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import { usePeers, useSyncNow } from "@/hooks/useDevices";
import { useServiceConfig } from "@/hooks/useServiceConfig";
import { statusReachable, useStatus } from "@/hooks/useStatus";
import { useTranslation } from "@/i18n";
import { useUi } from "@/store/ui";
import styles from "./SyncTab.module.css";

function focusSyncEnabledControl(): boolean {
  const control = document.getElementById("sync-enabled");
  if (control === null) return false;
  control.closest<HTMLElement>("[data-settings-search-target]")?.scrollIntoView({ behavior: "smooth", block: "center" });
  control.focus({ preventScroll: true });
  return true;
}

export function DeviceSyncSettings() {
  const { t } = useTranslation();
  const peers = usePeers();
  const sync = useSyncNow();
  const config = useServiceConfig();
  const service = useStatus(statusReachable);
  const [health, setHealth] = useState<PeerHealthMap>({});
  const setView = useUi((state) => state.setView);
  const setSettingsTab = useUi((state) => state.setSettingsTab);
  const count = peers.data?.length;
  const readiness = syncReadinessOf({ service, config, peers });
  const recovery = syncReadinessRecovery(readiness);
  const summary = connectionSummary({ serviceOffline: service.isError, serviceStarting: service.isPending, syncing: sync.isPending, peersLoaded: !peers.isPending, peersFailed: peers.isError, peers: peers.data ?? [], health });
  const partialFailure = sync.data?.some((result) => result.error !== null) ?? false;
  const syncNote = readiness !== "ready" ? <FieldFeedback state={syncReadinessIsLoading(readiness) ? "pending" : readiness === "disabled" || readiness === "no-peers" ? "neutral" : "warning"}>{syncReadinessMessage(readiness)}</FieldFeedback>
    : sync.isError ? <FieldFeedback state="error">{t("settings.sync.now.failed")}</FieldFeedback>
    : summary ? <FieldFeedback state={summary.status === "attention" ? "warning" : "pending"}>{summary.title}{summary.supportingLine ? ` ${summary.supportingLine}` : ""}</FieldFeedback>
    : partialFailure ? <FieldFeedback state="warning">{t("settings.sync.now.partial")}</FieldFeedback> : undefined;
  const peerLabel = peers.isError ? count === undefined ? t("settings.sync.paired.unavailable") : t("devices.syncReadiness.lastKnownCount", { n: count })
    : peers.isPending ? t("settings.sync.paired.checking")
    : count === undefined ? t("settings.sync.paired.unavailable")
    : count === 0 ? t("settings.sync.paired.none") : t("settings.sync.paired.count", { n: count });
  const recoveryAction = recovery === "retry-service" ? () => void service.refetch()
    : recovery === "retry-config" ? () => void config.refetch()
    : recovery === "retry-peers" ? () => void peers.refetch()
    : recovery === "enable-sync" ? () => { if (focusSyncEnabledControl()) return; setSettingsTab("device-sync"); setView("settings"); }
    : undefined;
  return <SettingsSchemaRenderer groups={[{
    id: "devices", title: "Devices", fields: [
      { kind: "custom", definition: settingDefinition("device-sync", "devices.own.rename.label"), content: <DeviceNameField showCurrentName /> },
      { kind: "custom", definition: settingDefinition("device-sync", "settings.sync.paired.title"), content: <div className={styles.pairedActions}><Badge variant={peers.isError ? "warn" : "secondary"}>{peerLabel}</Badge><Button variant="secondary" size="sm" icon="devices" label={t("settings.sync.paired.open")} onClick={() => setView("devices")} /></div> },
      { kind: "action", definition: settingDefinition("device-sync", "settings.sync.now.title"), visible: count !== undefined && count > 0, label: t(sync.isPending ? "settings.sync.now.pending" : "settings.sync.now.action"), icon: "refresh", disabled: sync.isPending || readiness !== "ready", busy: sync.isPending, note: syncNote, onAction: () => { if (readiness !== "ready" || sync.isPending) return; sync.mutate(undefined, { onSuccess: (results) => setHealth((previous) => noteSync(previous, results)) }); }, extraActions: recoveryAction ? <Button variant="ghost" size="sm" onClick={recoveryAction}>{t(recovery === "enable-sync" ? "settings.sync.now.showSetting" : "common.tryAgain")}</Button> : undefined },
    ],
  }]} />;
}
