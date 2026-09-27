import { Icon } from "@/components/ui/icon";
import { useId, useState } from "react";

import { FieldFeedback, SettingsRow } from "@/components/shared";
import { Badge, Button } from "@/components/ui";
import { DeviceNameField } from "@/features/devices";
import {
  connectionSummary,
  syncReadinessIsLoading,
  syncReadinessMessage,
  syncReadinessOf,
  syncReadinessRecovery,
} from "@/features/devices/model";
import { noteSync, type PeerHealthMap } from "@/features/devices/model/peerState";
import { Section } from "@/features/settings/components/Section";
import { usePeers, useSyncNow } from "@/hooks/useDevices";
import { useServiceConfig } from "@/hooks/useServiceConfig";
import { statusReachable, useStatus } from "@/hooks/useStatus";
import { useTranslation } from "@/i18n";
import { useUi } from "@/store/ui";
import styles from "./SyncTab.module.css";

function focusSyncEnabledControl(): boolean {
  const control = document.getElementById("sync-enabled");
  if (control === null) return false;
  control.closest<HTMLElement>("[data-settings-search-target]")
    ?.scrollIntoView({ behavior: "smooth", block: "center" });
  control.focus({ preventScroll: true });
  return true;
}

export function DeviceSyncSettings() {
  const { t } = useTranslation();
  const syncNoteId = useId();
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
  const summary = connectionSummary({
    serviceOffline: service.isError,
    serviceStarting: service.isPending,
    syncing: sync.isPending,
    peersLoaded: !peers.isPending,
    peersFailed: peers.isError,
    peers: peers.data ?? [],
    health,
  });
  const partialFailure = sync.data?.some((result) => result.error !== null) ?? false;

  const syncNote = readiness !== "ready" ? (
    <FieldFeedback state={syncReadinessIsLoading(readiness) ? "pending"
      : readiness === "disabled" || readiness === "no-peers" ? "neutral" : "warning"}>
      {syncReadinessMessage(readiness)}
    </FieldFeedback>
  ) : sync.isError ? (
    <FieldFeedback state="error">{t("settings.sync.now.failed")}</FieldFeedback>
  ) : summary ? (
    <FieldFeedback state={summary.status === "attention" ? "warning" : "pending"}>
      {summary.title}{summary.supportingLine ? ` ${summary.supportingLine}` : ""}
    </FieldFeedback>
  ) : partialFailure ? (
    <FieldFeedback state="warning">{t("settings.sync.now.partial")}</FieldFeedback>
  ) : undefined;

  return (
    <>
      <Section title="Devices">
        <SettingsRow
          title={t("devices.own.rename.label")}
        >
          <DeviceNameField showCurrentName />
        </SettingsRow>

        <SettingsRow
          title={t("settings.sync.paired.title")}
          help={t("settings.sync.paired.description")}
        >
          <div className={styles.pairedActions}>
            <Badge variant={peers.isError ? "warn" : "secondary"}>
              {peers.isError
                ? count === undefined
                  ? t("settings.sync.paired.unavailable")
                  : t("devices.syncReadiness.lastKnownCount", { n: count })
                : peers.isPending
                  ? t("settings.sync.paired.checking")
                  : count === undefined
                    ? t("settings.sync.paired.unavailable")
                  : count === 0
                    ? t("settings.sync.paired.none")
                    : t("settings.sync.paired.count", { n: count })}
            </Badge>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setView("devices")}
            >
              <Icon name="devices" aria-hidden="true" />
              {t("settings.sync.paired.open")}
            </Button>
          </div>
        </SettingsRow>

        {count !== undefined && count > 0 ? <SettingsRow
          title={t("settings.sync.now.title")}
          note={syncNote ? <span id={syncNoteId}>{syncNote}</span> : undefined}
        >
          <div className={styles.pairedActions}>
            <Button
              variant="secondary"
              size="sm"
              disabled={sync.isPending || readiness !== "ready"}
              aria-busy={sync.isPending || undefined}
              aria-describedby={syncNote ? syncNoteId : undefined}
              onClick={() => {
                if (readiness !== "ready" || sync.isPending) return;
                sync.mutate(undefined, {
                  onSuccess: (results) => setHealth((previous) => noteSync(previous, results)),
                });
              }}
            >
              <Icon name="refresh" aria-hidden="true" className={sync.isPending ? styles.spinner : undefined} />
              {t(sync.isPending ? "settings.sync.now.pending" : "settings.sync.now.action")}
            </Button>
            {recovery === "retry-service" ? (
              <Button variant="ghost" size="sm" onClick={() => void service.refetch()}>
                {t("common.tryAgain")}
              </Button>
            ) : recovery === "retry-config" ? (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => void config.refetch()}
              >
                {t("common.tryAgain")}
              </Button>
            ) : recovery === "retry-peers" ? (
              <Button variant="ghost" size="sm" onClick={() => void peers.refetch()}>
                {t("common.tryAgain")}
              </Button>
            ) : recovery === "enable-sync" && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => {
                  if (focusSyncEnabledControl()) return;
                  setSettingsTab("device-sync");
                  setView("settings");
                }}
              >
                {t("settings.sync.now.showSetting")}
              </Button>
            )}
          </div>
        </SettingsRow> : null}
      </Section>
    </>
  );
}
