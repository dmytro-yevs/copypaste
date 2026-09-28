/** Revocation permanently refuses the P2P pairing id (`PeerStore::revoke`,
 *  `CopyPaste-gbo`). The acknowledgement distinguishes that irreversible
 *  operation from recoverable unpairing. */
import { useEffect, useState } from "react";

import {
  AlertDialog,
  Checkbox,
  Label,
} from "@/components/ui";
import { StateView } from "@/components/shared/StateView";
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
    <AlertDialog
      open={peer !== null}
      onOpenChange={onOpenChange}
      title={<span className={styles.dialogTitle}>{t("devices.revoke.title", { name })}</span>}
      description={t("devices.revoke.body")}
      cancel={{ label: t("common.cancel"), disabled: pending }}
      action={{
        label: pending ? t("devices.revoke.pending") : t("devices.revoke.action"),
        variant: "danger",
        disabled: !acknowledged || pending,
        pending,
        onClick: () => {
          if (peer) void onConfirm(peer);
        },
      }}
    >
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
          <StateView
            mode="error"
            placement="inline"
            description={t("devices.revoke.failed", { error: toFriendly(error) })}
          />
        ) : null}

    </AlertDialog>
  );
}
