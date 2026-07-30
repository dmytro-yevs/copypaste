/**
 * "Add a device" — the other half of pairing, in three routes.
 *
 * **Scan**, which is the one that should be used: one QR carries the code and
 * the address together, so pairing is a gesture rather than a transcription.
 * **Pick**, when discovery has heard the device advertise itself — that fills
 * the address, leaving only the code. **Type**, which always works and is what
 * the other two degrade to.
 *
 * The bridge only keeps the pairing if a sync session with that device
 * succeeds, so a wrong code or an unreachable address leaves nothing behind.
 * That is what lets this dialog treat a failure as "nothing happened" and let
 * the user correct a typo, rather than having to offer an undo.
 *
 * The failure is rendered from the error *kind* (INV-12) — the address the user
 * typed is echoed back to them, but nothing from the daemon's message is.
 *
 * # Discovered devices are unverified
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
            <DialogTitle>Add a device</DialogTitle>
            <DialogDescription>
              Scan the code shown on the other device, or enter it by hand.
            </DialogDescription>
          </DialogHeader>

          {scanning ? (
            <Suspense
              fallback={
                <p className="text-center text-xs text-muted-foreground">
                  Starting the camera…
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
              Scan a code
            </Button>
          )}

          {nearby.length > 0 && (
            <div className="flex flex-col gap-s-1">
              <span className="text-sm font-medium">On this network</span>
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
                Unverified — these names are reported by the devices themselves
                and are only confirmed once pairing succeeds. Choosing one fills
                in the address; the code still has to come from that device.
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
              {rescan.isPending ? "Looking…" : "Look for nearby devices"}
            </Button>
          )}

          <div className="flex flex-col gap-s-1">
            <Label htmlFor="accept-code">Pairing code</Label>
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
            <Label htmlFor="accept-addr">Address</Label>
            <Input
              id="accept-addr"
              value={addr}
              placeholder="192.168.1.24:7420"
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => setAddr(event.target.value)}
              className="font-mono"
            />
            <p className="text-xs text-muted-foreground">
              Shown on the other device under the code, as host:port.
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
                  ? "Pairing isn't available in this build yet. It runs in the background service, which this build doesn't reach."
                  : `${toFriendly(accept.error)} Nothing was changed — check the code and the address and try again.`}
              </span>
            </p>
          )}

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={!ready || accept.isPending}>
              {accept.isPending && (
                <LoaderCircle aria-hidden="true" className="animate-spin" />
              )}
              {accept.isPending ? "Pairing…" : "Pair"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
