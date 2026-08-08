import { MonitorSmartphone, RefreshCw } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Row } from "@/components/settings/Row";
import { DeviceNameField } from "@/components/devices/DeviceNameField";
import { usePeers, useSyncNow } from "@/hooks/useDevices";
import { useTranslation } from "@/i18n";
import { isUnavailable } from "@/lib/errors";
import { useUi } from "@/store/ui";

export function SyncTab() {
  const { t } = useTranslation();
  const peers = usePeers();
  const sync = useSyncNow();
  const setView = useUi((s) => s.setView);

  const unavailable = peers.error !== null && isUnavailable(peers.error);
  const count = peers.data?.length ?? 0;

  return (
    <div className="flex flex-col">
      <Row
        title={t("devices.own.rename.label")}
        description={t("devices.own.rename.description")}
      >
        <DeviceNameField />
      </Row>
      <Row
        title={t("settings.sync.paired.title")}
        description={t("settings.sync.paired.description")}
      >
        <div className="flex items-center gap-s-2">
          <Badge
            variant={
              unavailable ? "secondary" : count > 0 ? "secondary" : "warn"
            }
          >
            {unavailable
              ? t("settings.sync.paired.unavailable")
              : count === 0
                ? t("settings.sync.paired.none")
                : t("settings.sync.paired.count", { n: count })}
          </Badge>
          <Button variant="outline" size="sm" onClick={() => setView("devices")}>
            <MonitorSmartphone aria-hidden="true" />
            {t("settings.sync.paired.manage")}
          </Button>
        </div>
      </Row>

      <Row
        title={t("settings.sync.now.title")}
        description={t("settings.sync.now.description")}
      >
        <Button
          variant="outline"
          size="sm"
          disabled={sync.isPending || count === 0 || unavailable}
          onClick={() => sync.mutate(undefined)}
        >
          <RefreshCw aria-hidden="true" />
          {t(sync.isPending ? "settings.sync.now.pending" : "settings.sync.now.action")}
        </Button>
      </Row>

      <Row
        title={t("settings.sync.cloud.title")}
        description={t("settings.sync.cloud.description")}
      >
        <Badge variant="secondary">{t("settings.sync.cloud.badge")}</Badge>
      </Row>
    </div>
  );
}
