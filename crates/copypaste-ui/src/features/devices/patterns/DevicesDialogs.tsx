import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  Button,
} from "@/components/ui";
import { InlineNotice } from "@/components/shared";
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
        onOpenChange={(open) => {
          if (!open && !unpairPending) onCloseUnpair();
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle className={styles.dialogTitle}>
              {t("devices.unpair.title", {
                name: unpairPeer?.name ?? t("devices.peer.thisDevice"),
              })}
            </AlertDialogTitle>
            <AlertDialogDescription>{t("devices.unpair.body")}</AlertDialogDescription>
          </AlertDialogHeader>
          <p className={styles.lost}>{t("devices.unpair.lost")}</p>
          {unpairError !== null ? (
            <InlineNotice tone="danger" role="alert">
              {t("devices.unpair.failed", { error: toFriendly(unpairError) })}
            </InlineNotice>
          ) : null}
          <AlertDialogFooter>
            <AlertDialogCancel disabled={unpairPending}>{t("common.cancel")}</AlertDialogCancel>
            <Button
              disabled={unpairPending}
              variant="danger"
              state={unpairPending ? "loading" : "normal"}
              onClick={() => {
                if (unpairPeer) void onUnpair(unpairPeer);
              }}
            >
              {unpairPending ? t("devices.unpair.pending") : t("devices.unpair.action")}
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
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
