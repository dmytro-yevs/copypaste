import { MonitorSmartphone, RefreshCw } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Row } from "@/components/settings/Row";
import { usePeers, useSyncNow } from "@/hooks/useDevices";
import { isUnavailable } from "@/lib/errors";
import { useUi } from "@/store/ui";

export function SyncTab() {
  const peers = usePeers();
  const sync = useSyncNow();
  const setView = useUi((s) => s.setView);

  const unavailable = peers.error !== null && isUnavailable(peers.error);
  const count = peers.data?.length ?? 0;

  return (
    <div className="flex flex-col">
      <Row
        title="Paired devices"
        description="Sync is peer-to-peer and end-to-end encrypted: devices exchange history directly, and nothing passes through a server."
      >
        <div className="flex items-center gap-s-2">
          {!unavailable && (
            <Badge variant={count > 0 ? "secondary" : "warn"}>
              {count === 0 ? "None" : `${count} paired`}
            </Badge>
          )}
          <Button variant="outline" size="sm" onClick={() => setView("devices")}>
            <MonitorSmartphone aria-hidden="true" />
            Manage devices
          </Button>
        </div>
      </Row>

      <Row
        title="Sync now"
        description="Runs a sync with every paired device. Devices also sync on their own when they see each other."
      >
        <Button
          variant="outline"
          size="sm"
          disabled={sync.isPending || count === 0 || unavailable}
          onClick={() => sync.mutate(undefined)}
        >
          <RefreshCw aria-hidden="true" />
          {sync.isPending ? "Syncing…" : "Sync now"}
        </Button>
      </Row>

      <Row
        title="Cloud sync"
        description="Not in this build. The service has no cloud-account command for the app to call, so signing in would be a form with nothing behind it."
      >
        <Badge variant="secondary">Unavailable</Badge>
      </Row>

      {unavailable && (
        <p className="py-s-3 text-sm text-muted-foreground">
          Device sync isn't available in this build — pairing runs in the
          background service, which this build doesn't reach.
        </p>
      )}
    </div>
  );
}
