/**
 * The bridge only keeps the pairing if a sync session with that device
 * succeeds, so a wrong code or an unreachable address leaves nothing behind.
 * That is what lets this dialog treat a failure as "nothing happened" rather
 * than having to offer an undo.
 *
 * The failure is rendered from the error *kind* (INV-12) — nothing from the
 * daemon's message is.
 *
 * INV-15: a name and an address off an mDNS record are what a device on the
 * network *claimed*. Only the Noise handshake proves any of it, so the list
 * says so rather than presenting them as known devices.
 */
import { type FormEvent, Suspense, lazy, useEffect, useState } from "react";
import { CircleAlert, LoaderCircle, QrCode as QrIcon, RefreshCw } from "lucide-react";

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
import { useDiscovered, usePairAccept, useRescan } from "@/hooks/useDevices";
import { useTranslation } from "@/i18n";
import { isUnavailable, toFriendly } from "@/lib/errors";
import type { PairingPayload } from "@/lib/pairing";

/**
 * Split out of the main bundle.
 *
 * `@zxing/browser` and its decoder are ~600 kB — more than the entire rest of
 * the app — and they are needed only when someone presses "Scan a code". A
 * clipboard manager must not pay a QR decoder's parse cost on every launch for
 * a screen most sessions never open.
 */
const QrScanner = lazy(async () => ({
  default: (await import("@/components/devices/QrScanner")).QrScanner,
}));

interface PairAcceptDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function PairAcceptDialog({ open, onOpenChange }: PairAcceptDialogProps) {
  const { t } = useTranslation();
  const accept = usePairAccept();
  const discovered = useDiscovered(open);
  const rescan = useRescan();
  const [code, setCode] = useState("");
  const [addr, setAddr] = useState("");
  const [scanning, setScanning] = useState(false);

  useEffect(() => {
    if (!open) {
      accept.reset();
      setCode("");
      setAddr("");
      // The camera stops with the dialog. Leaving `scanning` true would
      // re-open a stream the next time the dialog is shown, before the user
      // asked for one.
      setScanning(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  function submit(event: FormEvent) {
    event.preventDefault();
    accept.mutate({ code, addr }, { onSuccess: () => onOpenChange(false) });
  }

  /**
   * A scan is the whole pairing, so it submits rather than filling the form.
   * Making the user press a button after pointing a camera at a code is the
   * step the QR existed to remove.
   */
  function onScan(payload: PairingPayload) {
    setScanning(false);
    setCode(payload.code);
    setAddr(payload.addr);
    accept.mutate(payload, { onSuccess: () => onOpenChange(false) });
  }

  const nearby = (discovered.data ?? []).filter((device) => !device.paired);
  const ready = code.trim().length > 0 && addr.trim().length > 0;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[var(--modal-w-wide)]">
        <form onSubmit={submit} className="flex flex-col gap-4">
          <DialogHeader>
            <DialogTitle>{t("devices.accept.title")}</DialogTitle>
            <DialogDescription>
              {t("devices.accept.description")}
            </DialogDescription>
          </DialogHeader>

          {scanning ? (
            <Suspense
              fallback={
                <p className="text-center text-xs text-muted-foreground">
                  {t("devices.accept.startingCamera")}
                </p>
              }
            >
              <QrScanner onScan={onScan} />
            </Suspense>
          ) : (
            <Button
              type="button"
              variant="outline"
              onClick={() => setScanning(true)}
            >
              <QrIcon aria-hidden="true" />
              {t("devices.accept.scan")}
            </Button>
          )}

          {nearby.length > 0 && (
            <div className="flex flex-col gap-s-1">
              <span className="text-sm font-medium">
                {t("devices.accept.nearby")}
              </span>
              <ul className="flex flex-col gap-s-1">
                {nearby.map((device) => (
                  <li key={device.pairing_id}>
                    <Button
                      type="button"
                      variant="ghost"
                      className="w-full justify-start"
                      onClick={() => setAddr(device.addr)}
                    >
                      <span className="truncate">{device.name}</span>
                    </Button>
                  </li>
                ))}
              </ul>
              <p className="text-xs text-muted-foreground">
                {t("devices.accept.nearbyHint")}
              </p>
            </div>
          )}

          {open && nearby.length === 0 && !discovered.isPending && (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              disabled={rescan.isPending}
              onClick={() => rescan.mutate()}
            >
              <RefreshCw aria-hidden="true" />
              {t(
                rescan.isPending
                  ? "devices.accept.rescanning"
                  : "devices.accept.rescan",
              )}
            </Button>
          )}

          <div className="flex flex-col gap-s-1">
            <Label htmlFor="accept-code">{t("devices.accept.codeLabel")}</Label>
            <Input
              id="accept-code"
              value={code}
              autoComplete="off"
              spellCheck={false}
              // A password field would hide it from the person typing it in,
              // who is the one holding it legitimately. It is not stored.
              onChange={(event) => setCode(event.target.value)}
              className="font-mono"
            />
          </div>

          <div className="flex flex-col gap-s-1">
            <Label htmlFor="accept-addr">{t("devices.accept.addrLabel")}</Label>
            <Input
              id="accept-addr"
              value={addr}
              placeholder={t("devices.accept.addrPlaceholder")}
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => setAddr(event.target.value)}
              className="font-mono"
            />
            <p className="text-xs text-muted-foreground">
              {t("devices.accept.addrHint")}
            </p>
          </div>

          {accept.error !== null && (
            <p
              role="alert"
              className="flex items-start gap-2 rounded-md bg-err/15 px-s-2 py-s-2 text-sm text-err-strong"
            >
              <CircleAlert size={16} aria-hidden="true" className="mt-px shrink-0" />
              <span>
                {isUnavailable(accept.error)
                  ? t("devices.pairUnavailable")
                  : `${toFriendly(accept.error)} ${t("devices.accept.retryHint")}`}
              </span>
            </p>
          )}

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={!ready || accept.isPending}>
              {accept.isPending && (
                <LoaderCircle aria-hidden="true" className="animate-spin" />
              )}
              {t(accept.isPending ? "devices.accept.pairing" : "devices.accept.action")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
