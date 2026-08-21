import { useEffect, useId, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useTranslation } from "@/i18n";
import type { PreviewPairingInvite } from "@/lib/ipc";

interface PairingInviteDialogProps {
  invite: PreviewPairingInvite | null;
  onCancel: () => void;
}

export function PairingInviteDialog({
  invite,
  onCancel,
}: PairingInviteDialogProps) {
  const { t } = useTranslation();
  const [revealed, setRevealed] = useState(false);
  const [remaining, setRemaining] = useState(0);

  useEffect(() => {
    if (invite === null) {
      setRevealed(false);
      setRemaining(0);
      return;
    }

    setRevealed(false);
    setRemaining(invite.expires_in_secs);
    const startedAt = Date.now();
    const timer = globalThis.setInterval(() => {
      const elapsed = Math.floor((Date.now() - startedAt) / 1000);
      setRemaining(Math.max(invite.expires_in_secs - elapsed, 0));
    }, 1000);
    return () => globalThis.clearInterval(timer);
  }, [invite]);

  return (
    <Dialog open={invite !== null} onOpenChange={(open) => !open && onCancel()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("devices.pairing.inviteTitle")}</DialogTitle>
          <DialogDescription>
            {t("devices.pairing.inviteBody")}
          </DialogDescription>
        </DialogHeader>
        {invite !== null && (
          <>
            <div className="rounded-xl border border-border bg-background p-s-3">
              <div className="relative mx-auto w-full max-w-[18rem] overflow-hidden rounded-lg border border-divider bg-white p-4">
                <div
                  aria-label={t("devices.pairing.codeLabel")}
                  className={revealed ? "" : "pointer-events-none blur-md select-none"}
                  dangerouslySetInnerHTML={{ __html: invite.qr_svg }}
                />
                {!revealed && (
                  <div className="absolute inset-0 flex items-center justify-center bg-card/80">
                    <Button
                      type="button"
                      aria-label={t("devices.pairing.revealLabel")}
                      onClick={() => setRevealed(true)}
                    >
                      {t("devices.pairing.reveal")}
                    </Button>
                  </div>
                )}
              </div>
              <p className="mt-s-3 text-center text-xs text-muted-foreground">
                {t("devices.pairing.expires", { count: remaining })}
              </p>
            </div>
            <dl className="grid gap-s-3 text-sm">
              <div>
                <dt className="text-muted-foreground">
                  {t("devices.pairing.codeLabel")}
                </dt>
                <dd className="mt-1 rounded-md border border-divider bg-muted px-3 py-2 font-mono">
                  {invite.code}
                </dd>
              </div>
              <div>
                <dt className="text-muted-foreground">
                  {t("devices.pairing.addressLabel")}
                </dt>
                <dd className="mt-1 rounded-md border border-divider bg-muted px-3 py-2 font-mono break-all">
                  {invite.listen_addr}
                </dd>
              </div>
            </dl>
          </>
        )}
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onCancel}>
            {t("common.close")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

interface PairingJoinDialogProps {
  open: boolean;
  pending: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmit: (code: string, addr: string) => Promise<void>;
}

export function PairingJoinDialog({
  open,
  pending,
  onOpenChange,
  onSubmit,
}: PairingJoinDialogProps) {
  const { t } = useTranslation();
  const codeId = useId();
  const addrId = useId();
  const [code, setCode] = useState("");
  const [addr, setAddr] = useState("");
  const trimmedCode = code.trim();
  const trimmedAddr = addr.trim();

  useEffect(() => {
    if (open) return;
    setCode("");
    setAddr("");
  }, [open]);

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !pending && onOpenChange(nextOpen)}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("devices.pairing.joinTitle")}</DialogTitle>
          <DialogDescription>
            {t("devices.pairing.joinBody")}
          </DialogDescription>
        </DialogHeader>
        <form
          className="flex flex-col gap-s-4"
          onSubmit={async (event) => {
            event.preventDefault();
            if (!trimmedCode || !trimmedAddr) return;
            await onSubmit(trimmedCode, trimmedAddr);
          }}
        >
          <div className="flex flex-col gap-s-2">
            <Label htmlFor={codeId}>{t("devices.pairing.joinCode")}</Label>
            <Input
              id={codeId}
              value={code}
              disabled={pending}
              onChange={(event) => setCode(event.target.value)}
            />
          </div>
          <div className="flex flex-col gap-s-2">
            <Label htmlFor={addrId}>{t("devices.pairing.joinAddress")}</Label>
            <Input
              id={addrId}
              value={addr}
              disabled={pending}
              onChange={(event) => setAddr(event.target.value)}
            />
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              disabled={pending}
              onClick={() => onOpenChange(false)}
            >
              {t("common.cancel")}
            </Button>
            <Button
              type="submit"
              disabled={pending || !trimmedCode || !trimmedAddr}
            >
              {t("devices.pairing.joinAction")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
