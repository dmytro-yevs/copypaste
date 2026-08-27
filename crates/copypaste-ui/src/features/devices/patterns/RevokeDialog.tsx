/** Revocation permanently refuses the P2P pairing id (`PeerStore::revoke`,
 *  `CopyPaste-gbo`). The acknowledgement distinguishes that irreversible
 *  operation from recoverable unpairing. */
import { useEffect, useState } from "react";

import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  Button,
  Checkbox,
  Label,
} from "@/components/ui";
import { InlineNotice } from "@/components/shared";
import { useTranslation } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import type { PeerInfo } from "@/lib/ipc";
import styles from "./RevokeDialog.module.css";

interface RevokeDialogProps {
  peer: PeerInfo | null;
  pending: boolean;
  error: unknown | null;
  onOpenChange: (open: boolean) => void;
  onConfirm: (peer: PeerInfo) => Promise<void>;
}

export function RevokeDialog({
  peer,
  pending,
  error,
  onOpenChange,
  onConfirm,
}: RevokeDialogProps) {
  const { t } = useTranslation();
  const [acknowledged, setAcknowledged] = useState(false);

  useEffect(() => {
    setAcknowledged(false);
  }, [peer?.pairing_id]);

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
            disabled={pending}
            onCheckedChange={(state) => setAcknowledged(state === true)}
          />
          <Label htmlFor="revoke-ack" className={styles.label}>
            {t("devices.revoke.confirmLabel")}
          </Label>
        </div>
        {error !== null ? (
          <InlineNotice tone="danger" role="alert">
            {t("devices.revoke.failed", { error: toFriendly(error) })}
          </InlineNotice>
        ) : null}

        <AlertDialogFooter>
          <AlertDialogCancel disabled={pending}>{t("common.cancel")}</AlertDialogCancel>
          <Button
            disabled={!acknowledged || pending}
            aria-busy={pending || undefined}
            tone="danger"
            onClick={() => {
              if (peer) void onConfirm(peer);
            }}
          >
            {pending ? t("devices.revoke.pending") : t("devices.revoke.action")}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
