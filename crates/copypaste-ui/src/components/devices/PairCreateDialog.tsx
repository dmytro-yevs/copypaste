/**
 * The code is the Noise pre-shared key in transferable form: anyone holding it
 * can pair. Three rules follow, carried from manifest §3.3.1 even though the
 * handshake underneath is not v1's:
 *
 *  - **Hidden until revealed** (CopyPaste-1jms.2). Generated eagerly so there
 *    is no wait, covered until the user asks — the user is the one who knows
 *    whether a screen share or a shoulder is present.
 *  - **Shown exactly once.** The daemon does not keep it. Regenerating means a
 *    *new* code and re-blurs first: a fresh credential must not appear already
 *    visible (CopyPaste-crh3.21).
 *  - **Never logged, never toasted.**
 *
 * **INV-13: the QR's payload is never rendered as text.** `QrCode` draws to a
 * canvas for exactly that reason. The *code* below is a different string with
 * its own rule (hidden until revealed); the payload — code plus address — has
 * no revealed state at all.
 */
import { useEffect, useState } from "react";
import { Check, Copy, Eye, LoaderCircle, RefreshCw } from "lucide-react";

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
import { QrCode } from "@/components/devices/QrCode";
import { usePairCreate } from "@/hooks/useDevices";
import { useTranslation } from "@/i18n";
import { copyText } from "@/lib/ipc";
import { encodePairing } from "@/lib/pairing";
import { isUnavailable, toFriendly } from "@/lib/errors";
import { cn } from "@/lib/cn";

interface PairCreateDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function PairCreateDialog({ open, onOpenChange }: PairCreateDialogProps) {
  const { t } = useTranslation();
  const create = usePairCreate();
  const [name, setName] = useState("");
  const [revealed, setRevealed] = useState(false);
  const [qrRevealed, setQrRevealed] = useState(false);
  const [copied, setCopied] = useState(false);
  const [copyFailed, setCopyFailed] = useState(false);

  // Every open starts from nothing: no stale code, no stale reveal, no stale
  // error. A code from a previous episode must never be on screen.
  useEffect(() => {
    if (!open) {
      create.reset();
      setRevealed(false);
      setQrRevealed(false);
      setCopied(false);
      setCopyFailed(false);
    }
    // `create` is a stable mutation object; depending on it would reset on
    // every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const pairing = create.data ?? null;

  function generate() {
    // Re-blur *before* the new code arrives: it is a new credential.
    setRevealed(false);
    setQrRevealed(false);
    setCopied(false);
    setCopyFailed(false);
    create.mutate(name.trim() || t("devices.create.defaultName"));
  }

  /**
   * Through the backend, not `@tauri-apps/plugin-clipboard-manager`: the
   * capability file withholds `clipboard-manager:allow-write-text`, so the
   * plugin call this used to make always rejected and the empty `catch` around
   * it made the button do nothing, silently.
   *
   * A failure is *shown*. Not as a toast — the code is a credential and a toast
   * outlives the dialog — but the alternative is a user who walks to the other
   * device to paste something that was never copied.
   */
  async function copyCode() {
    if (!pairing) return;
    setCopyFailed(false);
    try {
      await copyText(pairing.code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // The raw failure is not rendered: `lib/errors` has already logged and
      // classified it, and nothing about a clipboard write is actionable
      // beyond "read it off the screen" (INV-12).
      setCopyFailed(true);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[var(--modal-w-wide)]">
        <DialogHeader>
          <DialogTitle>{t("devices.create.title")}</DialogTitle>
          <DialogDescription>{t("devices.create.description")}</DialogDescription>
        </DialogHeader>

        {pairing === null ? (
          <div className="flex flex-col gap-s-3">
            <div className="flex flex-col gap-s-1">
              <Label htmlFor="pair-name">{t("devices.create.nameLabel")}</Label>
              <Input
                id="pair-name"
                value={name}
                placeholder={t("devices.create.defaultName")}
                onChange={(event) => setName(event.target.value)}
              />
              <p className="text-xs text-muted-foreground">
                {t("devices.create.nameHint")}
              </p>
            </div>

            {create.error !== null && (
              <p
                role="alert"
                className="rounded-md bg-warn/15 px-s-2 py-s-2 text-sm text-warn-strong"
              >
                {isUnavailable(create.error)
                  ? t("devices.pairUnavailable")
                  : toFriendly(create.error)}
              </p>
            )}
          </div>
        ) : (
          <div className="flex flex-col gap-s-3">
            {pairing.listen_addr !== null && (
              <div className="flex flex-col items-center gap-s-2">
                {qrRevealed ? (
                  <>
                    <QrCode
                      value={encodePairing({
                        code: pairing.code,
                        addr: pairing.listen_addr,
                      })}
                      label={t("devices.create.qrLabel")}
                    />
                    <p className="text-center text-xs text-muted-foreground">
                      {t("devices.create.qrHint")}
                    </p>
                  </>
                ) : (
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => setQrRevealed(true)}
                  >
                    <Eye aria-hidden="true" />
                    {t("devices.create.revealQr")}
                  </Button>
                )}
              </div>
            )}

            <div className="flex flex-col gap-s-1">
              <span className="text-sm font-medium">
                {t("devices.create.codeLabel")}
                {pairing.listen_addr !== null && (
                  <span className="ml-1 font-normal text-muted-foreground">
                    {t("devices.create.codeCameraNote")}
                  </span>
                )}
              </span>
              <div className="relative">
                <output
                  aria-label={t(
                    revealed
                      ? "devices.create.codeLabel"
                      : "devices.create.codeHidden",
                  )}
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
                    aria-label={t("devices.create.revealLabel")}
                    onClick={() => setRevealed(true)}
                    className="absolute inset-0 flex items-center justify-center gap-2 rounded-md bg-card/60 text-sm font-medium outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
                  >
                    <Eye size={16} aria-hidden="true" />
                    {t("devices.create.reveal")}
                  </button>
                )}
              </div>
              <p className="text-xs text-muted-foreground">
                {t("devices.create.codeWarning")}
              </p>
            </div>

            <div className="flex flex-col gap-s-1">
              <span className="text-sm font-medium">
                {t("devices.create.addressLabel")}
              </span>
              <code className="rounded-md bg-muted px-s-2 py-s-1 font-mono text-sm">
                {pairing.listen_addr ?? t("devices.create.addressMissing")}
              </code>
              <p className="text-xs text-muted-foreground">
                {t("devices.create.addressHint")}
              </p>
            </div>

            {copyFailed && (
              <p
                role="alert"
                className="rounded-md bg-warn/15 px-s-2 py-s-2 text-sm text-warn-strong"
              >
                {t("devices.create.copyFailed")}
              </p>
            )}
          </div>
        )}

        <DialogFooter>
          {pairing !== null && (
            <Button variant="ghost" onClick={() => void copyCode()}>
              {copied ? <Check aria-hidden="true" /> : <Copy aria-hidden="true" />}
              {t(copied ? "devices.create.copied" : "devices.create.copyCode")}
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
            {t(
              create.isPending
                ? "devices.create.generating"
                : pairing === null
                  ? "devices.create.generate"
                  : "devices.create.regenerate",
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
