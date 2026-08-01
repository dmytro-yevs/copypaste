import { useEffect, useMemo, useRef, useState } from "react";
import { Camera, Check, Copy, X } from "lucide-react";
import { QRCodeSVG } from "qrcode.react";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
import { useAcceptPairing, useCreatePairing } from "@/hooks/useDevices";
import { useTranslation } from "@/i18n";
import { copyText, readPairingText, scanPairingQr, type PairingData } from "@/lib/ipc";
import { isAndroidPlatform } from "@/lib/platform";

type PairingFlow = "create" | "join";

interface PairingDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  flow: PairingFlow;
  initialAddress?: string | null;
  incomingUri: string | null;
  onIncomingUsed: () => void;
}

function pairingUri(pairing: PairingData): string {
  const url = new URL("copypaste://pair");
  url.searchParams.set("code", pairing.code);
  url.searchParams.set("id", pairing.pairing_id);
  if (pairing.listen_addr) url.searchParams.set("addr", pairing.listen_addr);
  return url.toString();
}

function readPairingUri(value: string): { code: string; addr: string; id: string } | null {
  try {
    const url = new URL(value);
    if (url.protocol !== "copypaste:" || url.hostname !== "pair") return null;
    const code = url.searchParams.get("code")?.trim() ?? "";
    const addr = url.searchParams.get("addr")?.trim() ?? "";
    const id = url.searchParams.get("id")?.trim() ?? "";
    return code && addr && id ? { code, addr, id } : null;
  } catch {
    return null;
  }
}

export function PairingDialog({ open, onOpenChange, flow, initialAddress, incomingUri, onIncomingUsed }: PairingDialogProps) {
  const { t } = useTranslation();
  const [pairing, setPairing] = useState<PairingData | null>(null);
  const [code, setCode] = useState("");
  const [addr, setAddr] = useState("");
  const [securityCode, setSecurityCode] = useState("");
  const [securityConfirmed, setSecurityConfirmed] = useState(false);
  const [copiedField, setCopiedField] = useState<string | null>(null);
  const create = useCreatePairing();
  const accept = useAcceptPairing();
  const android = isAndroidPlatform();
  const creationRequested = useRef(false);
  const copiedReset = useRef<ReturnType<typeof setTimeout> | null>(null);
  const qrValue = useMemo(() => (pairing ? pairingUri(pairing) : ""), [pairing]);
  const displayedCode = pairing?.pairing_id.slice(0, 6).toUpperCase() ?? "";
  const expectedCode = securityCode.trim().toUpperCase();

  function applyPairingDetails(decoded: { code: string; addr: string; id: string }) {
    setCode(decoded.code);
    setAddr(decoded.addr);
    setSecurityCode(decoded.id.slice(0, 6).toUpperCase());
    setSecurityConfirmed(false);
  }

  useEffect(() => {
    if (!incomingUri) return;
    const decoded = readPairingUri(incomingUri);
    if (decoded) applyPairingDetails(decoded);
    onIncomingUsed();
  }, [incomingUri, onIncomingUsed]);

  useEffect(() => {
    if (!open) {
      creationRequested.current = false;
      setPairing(null);
      setCode("");
      setAddr("");
      setSecurityCode("");
      setSecurityConfirmed(false);
      setCopiedField(null);
      return;
    }
    if (flow !== "create" || creationRequested.current) return;
    creationRequested.current = true;
    void createCode();
  // Starting a ceremony is deliberately tied to opening the create flow, not
  // to the mutation object's changing status.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, flow]);

  useEffect(() => {
    if (open && flow === "join" && initialAddress) setAddr(initialAddress);
  }, [flow, initialAddress, open]);

  useEffect(
    () => () => {
      if (copiedReset.current) clearTimeout(copiedReset.current);
    },
    [],
  );

  function close(next: boolean) {
    if (!next) {
      setPairing(null);
      setCode("");
      setAddr("");
      setSecurityCode("");
      setSecurityConfirmed(false);
      setCopiedField(null);
    }
    onOpenChange(next);
  }

  async function createCode() {
    const result = await create.mutateAsync(t("devices.own.name"));
    setPairing(result);
  }

  async function scan() {
    const value = await scanPairingQr();
    if (!value) return;
    const decoded = readPairingUri(value);
    if (decoded) {
      applyPairingDetails(decoded);
    } else {
      setCode(value);
      setSecurityCode("");
      setSecurityConfirmed(false);
    }
  }

  async function pasteDetails() {
    const value = await readPairingText();
    const decoded = readPairingUri(value);
    if (decoded) {
      applyPairingDetails(decoded);
      return;
    }
    setCode(value.trim());
    setSecurityCode("");
    setSecurityConfirmed(false);
  }

  async function copyField(field: string, value: string) {
    await copyText(value);
    setCopiedField(field);
    if (copiedReset.current) clearTimeout(copiedReset.current);
    copiedReset.current = setTimeout(() => setCopiedField(null), 1800);
  }

  function join() {
    accept.mutate(
      { code, addr },
      {
        onSuccess: () => close(false),
      },
    );
  }

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent className="!flex min-w-0 flex-col !gap-0 !overflow-hidden !p-0 sm:!max-w-lg">
        {flow === "create" && (
          <>
            <DialogHeader className="shrink-0 px-6 pt-6 pr-14 pb-s-3">
              <DialogTitle>{t("devices.pairing.showTitle")}</DialogTitle>
              <DialogDescription>{t("devices.pairing.showBody")}</DialogDescription>
            </DialogHeader>
            <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-6 pb-s-3">
              {pairing ? (
                <div className="flex min-w-0 flex-col gap-s-3">
                  <div className="mx-auto w-fit max-w-full rounded-lg bg-white p-s-2">
                    <QRCodeSVG
                      value={qrValue}
                      className="size-[min(12.5rem,calc(100vw-8rem))] max-w-full"
                      level="M"
                      includeMargin
                    />
                  </div>
                  <div className="flex min-w-0 flex-col gap-s-2">
                    <PairingField
                      label={t("devices.pairing.code")}
                      value={pairing.code}
                      copied={copiedField === "code"}
                      onCopy={() => void copyField("code", pairing.code)}
                    />
                    <PairingField
                      label={t("devices.pairing.securityCode")}
                      value={displayedCode}
                      copied={copiedField === "security"}
                      onCopy={() => void copyField("security", displayedCode)}
                    />
                    {pairing.listen_addr && (
                      <PairingField
                        label={t("devices.pairing.address")}
                        value={pairing.listen_addr}
                        copied={copiedField === "address"}
                        onCopy={() => void copyField("address", pairing.listen_addr!)}
                      />
                    )}
                  </div>
                  <p className="text-xs text-muted-foreground">{t("devices.pairing.verifyBody")}</p>
                </div>
              ) : (
                <p role="status" className="text-sm text-muted-foreground">{t("devices.pairing.creating")}</p>
              )}
            </div>
          </>
        )}

        {flow === "join" && (
          <>
            <DialogHeader className="shrink-0 px-6 pt-6 pr-14 pb-s-3">
              <DialogTitle>{t("devices.pairing.joinTitle")}</DialogTitle>
              <DialogDescription>{t("devices.pairing.joinBody")}</DialogDescription>
            </DialogHeader>
            <div
              aria-label={t("devices.pairing.joinTitle")}
              role="region"
              className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-6 pb-s-3"
            >
              <div className="flex flex-col gap-s-3">
                {android && (
                  <Button variant="outline" onClick={() => void scan()}>
                    <Camera aria-hidden="true" />
                    {t("devices.pairing.scan")}
                  </Button>
                )}
                <div className="flex flex-col gap-s-1">
                  <Label htmlFor="pairing-code">{t("devices.pairing.code")}</Label>
                  <Input id="pairing-code" value={code} onChange={(event) => setCode(event.target.value)} autoComplete="off" />
                </div>
                <div className="flex flex-col gap-s-1">
                  <Label htmlFor="pairing-address">{t("devices.pairing.address")}</Label>
                  <Input id="pairing-address" value={addr} onChange={(event) => setAddr(event.target.value)} autoComplete="off" />
                </div>
                <div className="flex flex-col gap-s-1">
                  <Label htmlFor="pairing-security-code">{t("devices.pairing.securityCode")}</Label>
                  <Input
                    id="pairing-security-code"
                    value={securityCode}
                    onChange={(event) => {
                      setSecurityCode(event.target.value);
                      setSecurityConfirmed(false);
                    }}
                    placeholder={t("devices.pairing.securityCodePlaceholder")}
                    autoComplete="off"
                  />
                  <p className="text-xs text-muted-foreground">{t("devices.pairing.verifyJoinBody")}</p>
                  <div className="flex items-center gap-s-1">
                    <Checkbox
                      id="pairing-security-ack"
                      checked={securityConfirmed}
                      onCheckedChange={(checked) => setSecurityConfirmed(checked === true)}
                    />
                    <Label htmlFor="pairing-security-ack" className="text-xs font-normal">
                      {t("devices.pairing.verifyConfirm")}
                    </Label>
                  </div>
                </div>
              </div>
            </div>
            <DialogFooter className="!flex min-h-[calc(var(--tap-min)+var(--s-3))] shrink-0 flex-row flex-wrap items-center gap-s-2 border-t border-divider bg-card px-6 py-s-2 pb-[max(var(--s-2),var(--inset-bottom))]">
              <Button variant="outline" onClick={() => void pasteDetails()}>
                <Copy aria-hidden="true" />
                {t("devices.pairing.pasteDetails")}
              </Button>
              <div className="ml-auto flex flex-wrap items-center justify-end gap-s-2">
                <Button variant="outline" onClick={() => close(false)}>
                  <X aria-hidden="true" />
                  {t("common.cancel")}
                </Button>
                <Button disabled={!code.trim() || !addr.trim() || expectedCode.length !== 6 || !securityConfirmed || accept.isPending} onClick={join}>
                  <Check aria-hidden="true" />
                  {t("devices.pairing.confirm")}
                </Button>
              </div>
            </DialogFooter>
          </>
        )}

        {flow === "create" && pairing && (
          <DialogFooter className="!flex min-h-[calc(var(--tap-min)+var(--s-3))] shrink-0 flex-row flex-wrap items-center gap-s-2 border-t border-divider bg-card px-6 py-s-2 pb-[max(var(--s-2),var(--inset-bottom))]">
            <Button variant="outline" onClick={() => void copyText(qrValue)}>
              <Copy aria-hidden="true" />
              {t("devices.pairing.copyDetails")}
            </Button>
            <Button className="ml-auto" onClick={() => close(false)}>
              <Check aria-hidden="true" />
              {t("common.done")}
            </Button>
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  );
}

function PairingField({
  label,
  value,
  copied,
  onCopy,
}: {
  label: string;
  value: string;
  copied: boolean;
  onCopy: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onCopy}
      aria-label={`${label}: ${copied ? "Copied" : "Copy"}`}
      className="group flex min-w-0 items-center gap-s-2 rounded-md border border-border bg-muted px-s-2 py-s-2 text-left transition-colors hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring focus-visible:outline-none"
    >
      <div className="min-w-0 flex-1">
        <p className="text-xs text-muted-foreground">{label}</p>
        <p className="mt-0.5 truncate font-mono text-xs text-foreground">{value}</p>
      </div>
      {copied ? (
        <Check aria-hidden="true" className="size-4 shrink-0 text-primary" />
      ) : (
        <Copy aria-hidden="true" className="size-4 shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100" />
      )}
    </button>
  );
}
