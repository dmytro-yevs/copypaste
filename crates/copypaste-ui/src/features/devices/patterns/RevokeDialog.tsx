/** Revocation permanently refuses the P2P pairing id (`PeerStore::revoke`,
 *  `CopyPaste-gbo`). The acknowledgement distinguishes that irreversible
 *  operation from recoverable unpairing. */
import { useEffect, useState } from "react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  Checkbox,
  Label,
} from "@/components/ui";
import { useTranslation } from "@/i18n";
import type { PeerInfo } from "@/lib/ipc";
import styles from "./RevokeDialog.module.css";

interface RevokeDialogProps {
  peer: PeerInfo | null;
  onOpenChange: (open: boolean) => void;
  onConfirm: (peer: PeerInfo) => void;
}

export function RevokeDialog({ peer, onOpenChange, onConfirm }: RevokeDialogProps) {
  const { t } = useTranslation();
  const [acknowledged, setAcknowledged] = useState(false);

  useEffect(() => {
    if (peer === null) setAcknowledged(false);
  }, [peer]);

  const name = peer?.name ?? t("devices.peer.thisDevice");

  return (
    <AlertDialog open={peer !== null} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle className={styles.dialogTitle}>
            {t("devices.revoke.title", { name })}
          </AlertDialogTitle>
          <AlertDialogDescription>{t("devices.revoke.body")}</AlertDialogDescription>
        </AlertDialogHeader>

        <ul className={styles.consequences}>
          <li>{t("devices.revoke.lostCode")}</li>
          <li>{t("devices.revoke.lostOneSided", { name })}</li>
          <li>{t("devices.revoke.keptHistory")}</li>
        </ul>

        <div className={styles.acknowledgement}>
          <Checkbox
            id="revoke-ack"
            checked={acknowledged}
            onCheckedChange={(state) => setAcknowledged(state === true)}
          />
          <Label htmlFor="revoke-ack" className={styles.label}>
            {t("devices.revoke.confirmLabel")}
          </Label>
        </div>

        <AlertDialogFooter>
          <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
          <AlertDialogAction
            disabled={!acknowledged}
            variant="danger"
            onClick={() => peer && onConfirm(peer)}
          >
            {t("devices.revoke.action")}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
