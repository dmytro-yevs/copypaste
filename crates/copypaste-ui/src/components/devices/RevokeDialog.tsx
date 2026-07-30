/**
 * The confirmation for the one device action that cannot be taken back.
 *
 * Unpairing and revoking do the same thing to *this* device — the key leaves
 * the listener's candidate list either way. They differ in what happens to the
 * pairing code, which is the long-term Noise pre-shared key: after an unpair it
 * still enrols the pairing, and after a revoke the pairing id is refused for
 * ever (`PeerStore::revoke`, `CopyPaste-gbo`). So this dialog names the code
 * first, and it does not merely ask "are you sure": CLAUDE.md rule 4 makes an
 * unrecoverable action one that has to say what is lost.
 *
 * It carries more weight than the unpair confirmation deliberately — an
 * acknowledgement that has to be ticked — because unpairing is recoverable and
 * a user who has learned that both dialogs are one click has learned the wrong
 * thing.
 */
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
} from "@/components/ui/alert-dialog";
import { buttonVariants } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import type { PeerInfo } from "@/lib/ipc";

interface RevokeDialogProps {
  /** The device to cut off, or `null` when the dialog is closed. */
  peer: PeerInfo | null;
  onOpenChange: (open: boolean) => void;
  onConfirm: (peer: PeerInfo) => void;
}

export function RevokeDialog({ peer, onOpenChange, onConfirm }: RevokeDialogProps) {
  const { t } = useTranslation();
  const [acknowledged, setAcknowledged] = useState(false);

  // Every open starts unticked, including the second open for a second device.
  useEffect(() => {
    if (peer === null) setAcknowledged(false);
  }, [peer]);

  const name = peer?.name ?? t("devices.peer.thisDevice");

  return (
    <AlertDialog open={peer !== null} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t("devices.revoke.title", { name })}</AlertDialogTitle>
          <AlertDialogDescription>{t("devices.revoke.body")}</AlertDialogDescription>
        </AlertDialogHeader>

        <ul className="flex list-disc flex-col gap-s-2 pl-5 text-sm text-muted-foreground">
          <li>{t("devices.revoke.lostCode")}</li>
          <li>{t("devices.revoke.lostOneSided", { name })}</li>
          <li>{t("devices.revoke.keptHistory")}</li>
        </ul>

        <div className="flex items-center gap-s-2">
          <Checkbox
            id="revoke-ack"
            checked={acknowledged}
            onCheckedChange={(state) => setAcknowledged(state === true)}
          />
          <Label htmlFor="revoke-ack" className="text-sm font-normal">
            {t("devices.revoke.confirmLabel")}
          </Label>
        </div>

        <AlertDialogFooter>
          <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
          <AlertDialogAction
            disabled={!acknowledged}
            className={cn(buttonVariants({ variant: "destructive" }))}
            onClick={() => peer && onConfirm(peer)}
          >
            {t("devices.revoke.action")}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
