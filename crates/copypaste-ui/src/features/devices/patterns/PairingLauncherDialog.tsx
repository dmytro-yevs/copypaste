import {
    useEffect,
    useId,
    useState,
    type RefObject,
} from "react";
import { useTranslation } from "react-i18next";

import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    Icon,
} from "@/components/ui";
import { FieldFeedback } from "@/components/shared";
import type { PairingController } from "@/features/pairing/hooks/usePairing";
import { PairingProgressCard } from "@/features/pairing/patterns/PairingProgressCard";
import {
    pairingClientErrorPresentation,
    pairingPresentation,
} from "@/features/pairing/model/pairingPresentation";
import { isAndroidPlatform } from "@/lib/platform";
import styles from "./PairingLauncherDialog.module.css";

type PairingFlow = "choices" | "host" | "join";

const ANDROID_PAIRING_BODY_ID = "copypaste-pairing-dialog-open";

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
    const showDescriptionId = useId();
    const joinDescriptionId = useId();
    const [flow, setFlow] = useState<PairingFlow>("choices");
    const [started, setStarted] = useState(false);
    const presentation = pairingPresentation(pairing.ceremony);
    const clientError = pairingClientErrorPresentation(pairing.error);
    const { semantics } = presentation;
    const lifecycleActive = pairing.isPending || semantics.active;
    const active = pairing.isPending || (clientError === null && semantics.active);
    const failed =
        clientError !== null ||
        (semantics.terminal && semantics.message_id !== "paired");
    const retryable =
        (clientError?.retry ?? semantics.retry) && pairing.canRetry;
    const confirmed =
        clientError === null &&
        semantics.terminal &&
        semantics.message_id === "paired";

    useEffect(() => {
        if (open) return;
        setFlow("choices");
        setStarted(false);
    }, [open]);

    useEffect(() => {
        if (!android || !open) return;
        const previousId = document.body.getAttribute("id");
        document.body.id = ANDROID_PAIRING_BODY_ID;
        return () => {
            if (previousId === null) {
                document.body.removeAttribute("id");
            } else {
                document.body.id = previousId;
            }
        };
    }, [android, open]);

    const close = (nextOpen: boolean) => {
        if (!nextOpen && lifecycleActive && pairing.pendingAction !== "cancel") {
            pairing.run("cancel");
        }
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
    const tryAgain = () => {
        pairing.retry();
    };

    const title =
        flow === "host"
            ? "Show pairing code"
            : flow === "join"
              ? "Enter pairing code"
              : "Connect a device";
    const description =
        flow === "choices"
            ? available
                ? preview
                    ? t("devices.pairing.previewUnavailable")
                    : "Choose how to open the protected pairing flow on this device."
                : "Protected pairing requires a current native CopyPaste build. Update or reopen the native app on both devices, then choose Connect a device again."
            : t("devices.pairing.previewUnavailable");

    return (
        <Dialog open={open} onOpenChange={close}>
            <DialogContent
                onCloseAutoFocus={(event) => {
                    if (!returnFocusRef?.current) return;
                    event.preventDefault();
                    returnFocusRef.current.focus();
                }}
            >
                <DialogHeader>
                    <DialogTitle>{title}</DialogTitle>
                    <DialogDescription>{description}</DialogDescription>
                </DialogHeader>

                {available && flow === "choices" ? (
                    <div className={styles.choices}>
                        <Button
                            type="button"
                            variant="ghost"
                            disabled={disabled}
                            aria-label="Show pairing code"
                            aria-describedby={showDescriptionId}
                            onClick={chooseHost}
                            className={styles.choice}
                        >
                            <span className={styles.well}>
                                <Icon name="qrCode" size="md" />
                            </span>
                            <span className={styles.copy}>
                                <strong>Show pairing code</strong>
                                <small id={showDescriptionId}>
                                    {preview
                                        ? t("devices.pairing.previewUnavailable")
                                        : "Open a protected code on this device."}
                                </small>
                            </span>
                            <Icon name="caretRight" size="sm" />
                        </Button>
                        <Button
                            type="button"
                            variant="ghost"
                            disabled={disabled}
                            aria-label={
                                android
                                    ? t("devices.pairing.join")
                                    : t("devices.pairing.joinTitle")
                            }
                            aria-describedby={joinDescriptionId}
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
                                <small id={joinDescriptionId}>
                                    {preview
                                        ? t("devices.pairing.previewUnavailable")
                                        : android
                                        ? t("devices.pairing.joinHint")
                                        : t("devices.pairing.joinBody")}
                                </small>
                            </span>
                            <Icon name="caretRight" size="sm" />
                        </Button>
                    </div>
                ) : null}

                {preview && flow !== "choices" ? (
                    <div className={styles.flow}>
                        <FieldFeedback state="warning">
                            {t("devices.pairing.previewUnavailable")}
                        </FieldFeedback>
                        <PairingProgressCard
                            pairing={pairing}
                            compact
                            showActions={false}
                        />
                    </div>
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
                        ) : failed && started && retryable ? (
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
