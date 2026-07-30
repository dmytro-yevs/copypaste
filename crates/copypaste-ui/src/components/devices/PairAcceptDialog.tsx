/**
 * "Enter a code" — the other half of pairing.
 *
 * The bridge only keeps the pairing if a sync session with that device
 * succeeds, so a wrong code or an unreachable address leaves nothing behind.
 * That is what lets this dialog treat a failure as "nothing happened" and let
 * the user correct a typo, rather than having to offer an undo.
 *
 * The failure is rendered from the error *kind* (INV-12) — the address the user
 * typed is echoed back to them, but nothing from the daemon's message is.
 */
import { type FormEvent, useEffect, useState } from "react";
import { CircleAlert, LoaderCircle } from "lucide-react";

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
import { usePairAccept } from "@/hooks/useDevices";
import { isUnavailable, toFriendly } from "@/lib/errors";

interface PairAcceptDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function PairAcceptDialog({ open, onOpenChange }: PairAcceptDialogProps) {
  const accept = usePairAccept();
  const [code, setCode] = useState("");
  const [addr, setAddr] = useState("");

  useEffect(() => {
    if (!open) {
      accept.reset();
      setCode("");
      setAddr("");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  function submit(event: FormEvent) {
    event.preventDefault();
    accept.mutate(
      { code, addr },
      { onSuccess: () => onOpenChange(false) },
    );
  }

  const ready = code.trim().length > 0 && addr.trim().length > 0;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <form onSubmit={submit} className="flex flex-col gap-4">
          <DialogHeader>
            <DialogTitle>Enter a pairing code</DialogTitle>
            <DialogDescription>
              Generate a code on the other device, then type it here along with
              the address it shows.
            </DialogDescription>
          </DialogHeader>

          <div className="flex flex-col gap-s-1">
            <Label htmlFor="accept-code">Pairing code</Label>
            <Input
              id="accept-code"
              value={code}
              autoFocus
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
