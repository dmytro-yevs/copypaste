import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui";
import { RevokeDialog } from "@/features/devices/patterns/RevokeDialog";
import { useTranslation } from "@/i18n";
import type { PeerInfo } from "@/lib/ipc";
import styles from "./DevicesDialogs.module.css";

interface DevicesDialogsProps {
  unpairPeer: PeerInfo | null;
  revokePeer: PeerInfo | null;
  onCloseUnpair: () => void;
  onUnpair: (peer: PeerInfo) => void;
  onCloseRevoke: () => void;
  onRevoke: (peer: PeerInfo) => void;
}

export function DevicesDialogs({
  unpairPeer,
  revokePeer,
  onCloseUnpair,
  onUnpair,
  onCloseRevoke,
  onRevoke,
}: DevicesDialogsProps) {
  const { t } = useTranslation();
  return (
    <>
      <AlertDialog open={unpairPeer !== null} onOpenChange={(open) => !open && onCloseUnpair()}>
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
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              variant="danger"
              onClick={() => {
                if (unpairPeer) onUnpair(unpairPeer);
                onCloseUnpair();
              }}
            >
              {t("devices.unpair.action")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <RevokeDialog peer={revokePeer} onOpenChange={(open) => !open && onCloseRevoke()} onConfirm={onRevoke} />
    </>
  );
}
