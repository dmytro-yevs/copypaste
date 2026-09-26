import { Icon } from "@/components/ui/icon";
import { useId, useState } from "react";

import { FieldFeedback, SettingsRow } from "@/components/shared";
import { Badge, Button } from "@/components/ui";
import { DeviceNameField } from "@/features/devices";
import { connectionSummary } from "@/features/devices/model";
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
  const deviceDescriptionId = useId();
  const peersDescriptionId = useId();
  const syncDescriptionId = useId();
  const peers = usePeers();
  const sync = useSyncNow();
  const config = useServiceConfig();
  const service = useStatus(statusReachable);
  const [health, setHealth] = useState<PeerHealthMap>({});
  const setView = useUi((state) => state.setView);
  const setSettingsTab = useUi((state) => state.setSettingsTab);
  const peersUnknown = peers.isError || service.isError;
  const count = peers.data?.length;
  const syncEnabled = config.data?.config.sync_enabled === true;
  const configUnknown = config.isError || (config.data === undefined && !config.isPending);
  const canSync = syncEnabled && !configUnknown && !peersUnknown &&
    !peers.isPending && !service.isPending && count !== undefined && count > 0;
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

  const syncNote = config.isPending
    ? <FieldFeedback state="pending">{t("settings.sync.now.configLoading")}</FieldFeedback>
    : configUnknown
      ? <FieldFeedback state="warning">{t("settings.sync.now.configUnavailable")}</FieldFeedback>
      : !syncEnabled
        ? <FieldFeedback state="neutral">{t("settings.sync.now.disabled")}</FieldFeedback>
        : peersUnknown
          ? <FieldFeedback state="warning">{t("settings.sync.now.peersUnavailable")}</FieldFeedback>
          : count === 0
            ? <FieldFeedback state="neutral">{t("settings.sync.now.noPeers")}</FieldFeedback>
            : sync.isError
              ? <FieldFeedback state="error">{t("settings.sync.now.failed")}</FieldFeedback>
              : summary
                ? <FieldFeedback state={summary.status === "attention" ? "warning" : "pending"}>
                      {summary.title}{summary.supportingLine ? ` ${summary.supportingLine}` : ""}
                    </FieldFeedback>
                : partialFailure
                  ? <FieldFeedback state="warning">{t("settings.sync.now.partial")}</FieldFeedback>
                  : undefined;

  return (
    <>
      <Section title="This device">
        <SettingsRow
          title={t("devices.own.rename.label")}
          descriptionId={deviceDescriptionId}
          description={t("devices.own.rename.description")}
        >
          <DeviceNameField showCurrentName descriptionId={deviceDescriptionId} />
        </SettingsRow>
      </Section>

      <Section title="Nearby devices">
        <SettingsRow
          title={t("settings.sync.paired.title")}
          descriptionId={peersDescriptionId}
          description={t("settings.sync.paired.description")}
        >
          <div className={styles.pairedActions}>
            <Badge variant={peersUnknown ? "warn" : "secondary"}>
              {peersUnknown
                  ? t("settings.sync.paired.unavailable")
                : peers.isPending
                  ? t("settings.sync.paired.checking")
                  : count === undefined
                    ? t("settings.sync.paired.checking")
                  : count === 0
                    ? t("settings.sync.paired.none")
                    : t("settings.sync.paired.count", { n: count })}
            </Badge>
            <Button
              variant="secondary"
              size="sm"
              aria-describedby={peersDescriptionId}
              onClick={() => setView("devices")}
            >
              <Icon name="devices" aria-hidden="true" />
              {t("settings.sync.paired.manage")}
            </Button>
          </div>
        </SettingsRow>

        <SettingsRow
          title={t("settings.sync.now.title")}
          descriptionId={syncDescriptionId}
          description={t("settings.sync.now.description")}
          note={syncNote}
        >
          <div className={styles.pairedActions}>
            <Button
              variant="secondary"
              size="sm"
              disabled={sync.isPending || !canSync}
              aria-busy={sync.isPending || undefined}
              aria-describedby={syncDescriptionId}
              onClick={() => sync.mutate(undefined, {
                onSuccess: (results) => setHealth((previous) => noteSync(previous, results)),
              })}
            >
              <Icon name="refresh" aria-hidden="true" className={sync.isPending ? styles.spinner : undefined} />
              {t(sync.isPending ? "settings.sync.now.pending" : "settings.sync.now.action")}
            </Button>
            {configUnknown ? (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => void config.refetch()}
              >
                {t("common.tryAgain")}
              </Button>
            ) : !config.isPending && !syncEnabled && (
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
        </SettingsRow>
      </Section>
    </>
  );
}
