import { Icon } from "@/components/ui/icon";
import { useId } from "react";

import { FieldFeedback, SettingsRow } from "@/components/shared";
import { Badge, Button } from "@/components/ui";
import { DeviceNameField } from "@/features/devices";
import { Section } from "@/features/settings/components/Section";
import { usePeers, useSyncNow } from "@/hooks/useDevices";
import { useTranslation } from "@/i18n";
import { useUi } from "@/store/ui";
import styles from "./SyncTab.module.css";

export function DeviceSyncSettings() {
  const { t } = useTranslation();
  const deviceDescriptionId = useId();
  const peersDescriptionId = useId();
  const syncDescriptionId = useId();
  const peers = usePeers();
  const sync = useSyncNow();
  const setView = useUi((state) => state.setView);
  const peersUnknown = peers.isError;
  const count = peers.data?.length;

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
            <Badge variant={peersUnknown || count === 0 ? "warn" : "secondary"}>
              {peers.isPending
                ? "Checking…"
                : peersUnknown
                  ? t("settings.sync.paired.unavailable")
                  : count === undefined
                    ? "Checking…"
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
          note={sync.isError ? (
            <FieldFeedback state="error">Sync couldn’t start.</FieldFeedback>
          ) : undefined}
        >
          <Button
            variant="secondary"
            size="sm"
            disabled={sync.isPending || count === 0 || count === undefined || peersUnknown}
            aria-busy={sync.isPending || undefined}
            aria-describedby={syncDescriptionId}
            onClick={() => sync.mutate(undefined)}
          >
            <Icon name="refresh" aria-hidden="true" className={sync.isPending ? styles.spinner : undefined} />
            {t(sync.isPending ? "settings.sync.now.pending" : "settings.sync.now.action")}
          </Button>
        </SettingsRow>
      </Section>
    </>
  );
}
