import {
  AlertDialog,
} from "@/components/ui";
import { StateView } from "@/components/shared/StateView";
import { RevokeDialog } from "@/features/devices/patterns/RevokeDialog";
import { useTranslation } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import type { PeerInfo } from "@/lib/ipc";
import styles from "./DevicesDialogs.module.css";

interface DevicesDialogsProps {
  unpairPeer: PeerInfo | null;
  revokePeer: PeerInfo | null;
  unpairPending: boolean;
  unpairError: unknown | null;
  revokePending: boolean;
  revokeError: unknown | null;
  onCloseUnpair: () => void;
  onUnpair: (peer: PeerInfo) => Promise<void>;
  onCloseRevoke: () => void;
  onRevoke: (peer: PeerInfo) => Promise<void>;
}

export function DevicesDialogs({
  unpairPeer,
  revokePeer,
  unpairPending,
  unpairError,
  revokePending,
  revokeError,
  onCloseUnpair,
  onUnpair,
  onCloseRevoke,
  onRevoke,
}: DevicesDialogsProps) {
  const { t } = useTranslation();
  return (
    <>
      <AlertDialog
        open={unpairPeer !== null}
        title={(
          <span className={styles.dialogTitle}>
            {t("devices.unpair.title", {
              name: unpairPeer?.name ?? t("devices.peer.thisDevice"),
            })}
          </span>
        )}
        description={t("devices.unpair.body")}
        cancel={{ label: t("common.cancel"), disabled: unpairPending }}
        action={{
          label: unpairPending ? t("devices.unpair.pending") : t("devices.unpair.action"),
          variant: "danger",
          pending: unpairPending,
          onClick: () => {
            if (unpairPeer) void onUnpair(unpairPeer);
          },
        }}
        onOpenChange={(open) => {
          if (!open && !unpairPending) onCloseUnpair();
        }}
      >
        <p className={styles.lost}>{t("devices.unpair.lost")}</p>
        {unpairError !== null ? (
          <StateView
            mode="error"
            placement="inline"
            description={t("devices.unpair.failed", { error: toFriendly(unpairError) })}
          />
        ) : null}
      </AlertDialog>

      <RevokeDialog
        peer={revokePeer}
        pending={revokePending}
        error={revokeError}
        onOpenChange={(open) => {
          if (!open && !revokePending) onCloseRevoke();
        }}
        onConfirm={onRevoke}
      />
    </>
  );
}
