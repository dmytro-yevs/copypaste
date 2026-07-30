/**
 * "Pair a new device" — mint a code on this device and read it out on the
 * other one.
 *
 * The code is the Noise pre-shared key in transferable form: anyone holding it
 * can pair. Three rules follow from that, all carried from manifest §3.3.1
 * even though the handshake underneath is not v1's:
 *
 *  - **Hidden until revealed** (CopyPaste-1jms.2). It is generated eagerly so
 *    there is no wait, and covered until the user asks for it — a screen share
 *    or a shoulder is the threat, and the user is the one who knows whether
 *    either is present.
 *  - **Shown exactly once.** The daemon does not keep it, so the dialog says
 *    so before the user closes it. Regenerating means a *new* code, and
 *    re-blurs first: a fresh credential must not appear already visible
 *    (CopyPaste-crh3.21).
 *  - **Never logged, never toasted.** It exists in this component's state and
 *    goes when the dialog does.
 */
import { useEffect, useState } from "react";
import { Check, Copy, Eye, LoaderCircle, RefreshCw } from "lucide-react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

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
import { usePairCreate } from "@/hooks/useDevices";
import { isUnavailable, toFriendly } from "@/lib/errors";
import { cn } from "@/lib/cn";

interface PairCreateDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function PairCreateDialog({ open, onOpenChange }: PairCreateDialogProps) {
  const create = usePairCreate();
  const [name, setName] = useState("");
  const [revealed, setRevealed] = useState(false);
  const [copied, setCopied] = useState(false);

  // Every open starts from nothing: no stale code, no stale reveal, no stale
  // error. A code from a previous episode must never be on screen.
  useEffect(() => {
    if (!open) {
      create.reset();
      setRevealed(false);
      setCopied(false);
    }
    // `create` is a stable mutation object; depending on it would reset on
    // every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const pairing = create.data ?? null;

  function generate() {
    // Re-blur *before* the new code arrives: it is a new credential.
    setRevealed(false);
    setCopied(false);
    create.mutate(name.trim() || "unnamed device");
  }

  async function copyCode() {
    if (!pairing) return;
    try {
      await writeText(pairing.code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Nothing is reported: the code is on screen, and a failed clipboard
      // write must not put it anywhere else.
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[var(--modal-w-wide)]">
        <DialogHeader>
          <DialogTitle>Pair a new device</DialogTitle>
          <DialogDescription>
            Generate a code here, then enter it on the other device under
            Devices → Enter a code.
          </DialogDescription>
        </DialogHeader>

        {pairing === null ? (
          <div className="flex flex-col gap-s-3">
            <div className="flex flex-col gap-s-1">
              <Label htmlFor="pair-name">Name for the other device</Label>
              <Input
                id="pair-name"
                value={name}
                placeholder="unnamed device"
                onChange={(event) => setName(event.target.value)}
              />
              <p className="text-xs text-muted-foreground">
                Used until that device tells us its own name.
              </p>
            </div>

            {create.error !== null && (
              <p
                role="alert"
                className="rounded-md bg-warn/15 px-s-2 py-s-2 text-sm text-warn-strong"
              >
                {isUnavailable(create.error)
                  ? "Pairing isn't available in this build yet. It runs in the background service, which this build doesn't reach."
                  : toFriendly(create.error)}
              </p>
            )}
          </div>
        ) : (
          <div className="flex flex-col gap-s-3">
            <div className="flex flex-col gap-s-1">
              <span className="text-sm font-medium">Pairing code</span>
              <div className="relative">
                <output
                  aria-label={
                    revealed
                      ? "Pairing code"
                      : "Pairing code, hidden. Activate reveal to show it."
                  }
                  className={cn(
                    "block rounded-md border border-border bg-muted px-s-3 py-s-3 font-mono text-lg tracking-wider break-all select-text",
                    !revealed && "blur-sm select-none",
                  )}
                >
                  {/* Rendered either way, because the reveal must not depend on
                      a second round trip — but unreadable until asked for. */}
                  {pairing.code}
                </output>
                {!revealed && (
                  <button
                    type="button"
                    aria-label="Reveal the pairing code"
                    onClick={() => setRevealed(true)}
                    className="absolute inset-0 flex items-center justify-center gap-2 rounded-md bg-card/60 text-sm font-medium outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
                  >
                    <Eye size={16} aria-hidden="true" />
                    Click to reveal
                  </button>
                )}
              </div>
              <p className="text-xs text-muted-foreground">
                Anyone with this code can pair with this device. It is shown once
                and cannot be retrieved again — generate a new one if you lose
                it.
              </p>
            </div>

            <div className="flex flex-col gap-s-1">
              <span className="text-sm font-medium">This device's address</span>
              <code className="rounded-md bg-muted px-s-2 py-s-1 font-mono text-sm">
                {pairing.listen_addr ?? "Not reachable — check that sync is enabled"}
              </code>
              <p className="text-xs text-muted-foreground">
                Enter this alongside the code on the other device.
              </p>
            </div>
          </div>
        )}

        <DialogFooter>
          {pairing !== null && (
            <Button variant="ghost" onClick={() => void copyCode()}>
              {copied ? <Check aria-hidden="true" /> : <Copy aria-hidden="true" />}
              {copied ? "Copied" : "Copy code"}
            </Button>
          )}
          <Button
            variant={pairing === null ? "default" : "outline"}
            onClick={generate}
            disabled={create.isPending}
          >
            {create.isPending ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" />
            ) : pairing === null ? null : (
              <RefreshCw aria-hidden="true" />
            )}
            {create.isPending
              ? "Generating…"
              : pairing === null
                ? "Generate code"
                : "Generate a new code"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
