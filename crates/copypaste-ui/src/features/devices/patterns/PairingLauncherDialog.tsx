import {
    useEffect,
    useMemo,
    useState,
    type FormEvent,
    type RefObject,
} from "react";
import { useTranslation } from "react-i18next";
import { z } from "zod";

import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    Icon,
    Input,
    Label,
} from "@/components/ui";
import { FieldFeedback } from "@/components/shared";
import type { PairingController } from "@/features/pairing/hooks/usePairing";
import { PairingProgressCard } from "@/features/pairing/patterns/PairingProgressCard";
import { isPairingActive } from "@/features/pairing/model/pairingSession";
import { isAndroidPlatform } from "@/lib/platform";
import styles from "./PairingLauncherDialog.module.css";

type PairingFlow = "choices" | "host" | "join";

const codeSchema = z
    .string()
    .trim()
    .regex(/^\d{3}\s?\d{3}$/)
    .transform((value) => value.replace(/\s/g, ""));
const addressSchema = z
    .string()
    .trim()
    .max(255)
    .regex(/^(?:\[[^\]]+\]|[^:\s]+):\d{1,5}$/)
    .refine(
        (value) => Number(value.slice(value.lastIndexOf(":") + 1)) <= 65_535,
    );

interface PairingLauncherDialogProps {
    open: boolean;
    available: boolean;
    preview: boolean;
    disabled: boolean;
    pairing: PairingController;
    onOpenChange: (open: boolean) => void;
    onCreate: () => void;
    onJoin: () => void;
    returnFocusRef?: RefObject<HTMLButtonElement | null>;
}

export function PairingLauncherDialog({
    open,
    available,
    preview,
    disabled,
    pairing,
    onOpenChange,
    onCreate,
    onJoin,
    returnFocusRef,
}: PairingLauncherDialogProps) {
    const { t } = useTranslation();
    const android = isAndroidPlatform();
    const [flow, setFlow] = useState<PairingFlow>("choices");
    const [code, setCode] = useState("");
    const [address, setAddress] = useState("");
    const [started, setStarted] = useState(false);
    const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">(
        "idle",
    );
    const pairingState = pairing.ceremony?.state ?? "idle";
    const active = isPairingActive(pairingState) || pairing.isPending;
    const failed =
        pairing.error !== null ||
        pairingState === "failed" ||
        pairingState === "rejected" ||
        pairingState === "timed_out" ||
        pairingState === "cancelled";
    const confirmed = pairingState === "confirmed";
    const normalized = useMemo(
        () => ({
            code: codeSchema.safeParse(code),
            address: addressSchema.safeParse(address),
        }),
        [address, code],
    );
    const formValid = normalized.code.success && normalized.address.success;

    useEffect(() => {
        if (open) return;
        setFlow("choices");
        setCode("");
        setAddress("");
        setStarted(false);
        setCopyState("idle");
    }, [open]);

    const close = (nextOpen: boolean) => {
        if (!nextOpen && active) pairing.run("cancel");
        onOpenChange(nextOpen);
    };
    const chooseHost = () => {
        if (!preview) {
            onOpenChange(false);
            onCreate();
            return;
        }
        setFlow("host");
        setStarted(true);
        setCopyState("idle");
        pairing.startPreviewCreate();
    };
    const chooseJoin = () => {
        if (!preview) {
            onOpenChange(false);
            onJoin();
            return;
        }
        setFlow("join");
        setStarted(false);
    };
    const submitJoin = (event: FormEvent) => {
        event.preventDefault();
        if (!normalized.code.success || !normalized.address.success) return;
        setStarted(true);
        void pairing.submitPreviewJoin(
            normalized.code.data,
            normalized.address.data,
        );
    };
    const copyInvite = async () => {
        const invite = pairing.previewInvite;
        if (!invite) return;
        try {
            await navigator.clipboard.writeText(
                `${invite.code}\n${invite.listen_addr}`,
            );
            setCopyState("copied");
        } catch {
            setCopyState("failed");
        }
    };
    const tryAgain = () => {
        if (flow === "host") {
            pairing.startPreviewCreate();
            return;
        }
        setStarted(false);
    };

    const title =
        flow === "host"
            ? "Show pairing code"
            : flow === "join"
              ? "Enter pairing code"
              : "Connect a device";

    return (
        <Dialog open={open} onOpenChange={close}>
            <DialogContent
                overlayAriaLabel="Dismiss pairing dialog"
                onCloseAutoFocus={(event) => {
                    if (!returnFocusRef?.current) return;
                    event.preventDefault();
                    returnFocusRef.current.focus();
                }}
            >
                <DialogHeader>
                    <DialogTitle>{title}</DialogTitle>
                    <DialogDescription>
                        {flow === "choices"
                            ? available
                                ? preview
                                    ? "Choose whether this preview shows a code or connects with one."
                                    : "Choose how to open the protected pairing flow on this device."
                                : "Protected pairing requires a current native CopyPaste build. Update or reopen the native app on both devices, then choose Connect a device again."
                            : flow === "host"
                              ? "Use either the QR code or the short code and address on the other device."
                              : "Paste the short code and address shown on the other device."}
                    </DialogDescription>
                </DialogHeader>

                {available && flow === "choices" ? (
                    <div className={styles.choices}>
                        <Button
                            type="button"
                            variant="ghost"
                            disabled={disabled}
                            onClick={chooseHost}
                            className={styles.choice}
                        >
                            <span className={styles.well}>
                                <Icon name="qrCode" size="md" />
                            </span>
                            <span className={styles.copy}>
                                <strong>Show pairing code</strong>
                                <small>
                                    {preview
                                        ? "Show a QR code, short code and address."
                                        : "Open a protected code on this device."}
                                </small>
                            </span>
                            <Icon name="caretRight" size="sm" />
                        </Button>
                        <Button
                            type="button"
                            variant="ghost"
                            disabled={disabled}
                            onClick={chooseJoin}
                            className={styles.choice}
                        >
                            <span className={styles.well}>
                                <Icon
                                    name={android ? "scan" : "key"}
                                    size="md"
                                />
                            </span>
                            <span className={styles.copy}>
                                <strong>
                                    {android
                                        ? t("devices.pairing.join")
                                        : t("devices.pairing.joinTitle")}
                                </strong>
                                <small>
                                    {android
                                        ? t("devices.pairing.joinHint")
                                        : t("devices.pairing.joinBody")}
                                </small>
                            </span>
                            <Icon name="caretRight" size="sm" />
                        </Button>
                    </div>
                ) : null}

                {preview && flow === "host" ? (
                    <div className={styles.flow}>
                        {pairing.previewInvite ? (
                            <div className={styles.invite}>
                                <img
                                    className={styles.qr}
                                    src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(pairing.previewInvite.qr_svg)}`}
                                    alt="Pairing QR code"
                                />
                                <dl className={styles.inviteDetails}>
                                    <div>
                                        <dt>Pairing code</dt>
                                        <dd>{pairing.previewInvite.code}</dd>
                                    </div>
                                    <div>
                                        <dt>Address</dt>
                                        <dd>
                                            {pairing.previewInvite.listen_addr}
                                        </dd>
                                    </div>
                                </dl>
                                <Button
                                    type="button"
                                    size="sm"
                                    variant="secondary"
                                    onClick={() => void copyInvite()}
                                >
                                    <Icon name="copy" aria-hidden="true" />
                                    {copyState === "copied"
                                        ? "Copied"
                                        : "Copy code and address"}
                                </Button>
                                {copyState === "failed" ? (
                                    <FieldFeedback state="error">
                                        Couldn’t copy. Select the code and
                                        address instead.
                                    </FieldFeedback>
                                ) : null}
                            </div>
                        ) : null}
                        <PairingProgressCard
                            pairing={pairing}
                            compact
                            showActions={false}
                        />
                    </div>
                ) : null}

                {preview && flow === "join" ? (
                    started ? (
                        <div className={styles.flow}>
                            <PairingProgressCard
                                pairing={pairing}
                                compact
                                showActions={false}
                            />
                        </div>
                    ) : (
                        <form className={styles.joinForm} onSubmit={submitJoin}>
                            <Label htmlFor="pairing-code">Pairing code</Label>
                            <Input
                                id="pairing-code"
                                value={code}
                                inputMode="numeric"
                                autoComplete="one-time-code"
                                placeholder="482 916"
                                maxLength={7}
                                state={
                                    code && !normalized.code.success
                                        ? "invalid"
                                        : undefined
                                }
                                onChange={(event) =>
                                    setCode(event.target.value)
                                }
                            />
                            <Label htmlFor="pairing-address">Address</Label>
                            <Input
                                id="pairing-address"
                                value={address}
                                autoCapitalize="none"
                                autoCorrect="off"
                                placeholder="192.168.1.20:49200"
                                maxLength={255}
                                state={
                                    address && !normalized.address.success
                                        ? "invalid"
                                        : undefined
                                }
                                onChange={(event) =>
                                    setAddress(event.target.value)
                                }
                            />
                            <Button
                                type="submit"
                                size="md"
                                disabled={!formValid}
                                className={styles.connect}
                            >
                                <Icon name="shieldCheck" aria-hidden="true" />
                                Connect
                            </Button>
                        </form>
                    )
                ) : null}

                {preview && flow !== "choices" ? (
                    <div className={styles.footerActions}>
                        {active ? (
                            <Button
                                type="button"
                                size="sm"
                                variant="secondary"
                                disabled={pairing.pendingAction === "cancel"}
                                onClick={() => pairing.run("cancel")}
                            >
                                {pairing.pendingAction === "cancel"
                                    ? "Cancelling…"
                                    : "Cancel"}
                            </Button>
                        ) : failed && started ? (
                            <Button
                                type="button"
                                size="sm"
                                variant="primary"
                                onClick={tryAgain}
                            >
                                <Icon name="refresh" aria-hidden="true" />
                                Try again
                            </Button>
                        ) : confirmed ? (
                            <Button
                                type="button"
                                size="sm"
                                onClick={() => close(false)}
                            >
                                Done
                            </Button>
                        ) : (
                            <Button
                                type="button"
                                size="sm"
                                variant="ghost"
                                onClick={() => setFlow("choices")}
                            >
                                Back
                            </Button>
                        )}
                    </div>
                ) : null}
            </DialogContent>
        </Dialog>
    );
}
