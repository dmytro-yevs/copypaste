import { Button, Icon } from "@/components/ui";
import type { PairingController } from "@/features/pairing/hooks/usePairing";
import { PairingProgressCard } from "@/features/pairing/patterns/PairingProgressCard";
import styles from "./DiscoveryPairingFooter.module.css";

export type DiscoveryConnectState = "idle" | "pending" | "success" | "error";

export function DiscoveryPairingFooter({
    deviceName,
    state,
    disabled,
    pairing,
    onConnect,
}: {
    deviceName: string;
    state: DiscoveryConnectState;
    disabled: boolean;
    pairing: PairingController;
    onConnect: () => void;
}) {
    const cancelling = pairing.pendingAction === "cancel";

    return (
        <section
            className={styles.root}
            aria-label={`Protected pairing with ${deviceName}`}
        >
            {state === "idle" ? (
                <p className={styles.hint}>
                    Pairing opens in a protected surface before this device is
                    trusted.
                </p>
            ) : state === "success" ? (
                <p className={styles.paired} role="status">
                    <span className={styles.pairedLayout}>
                        <Icon name="checkCircle" aria-hidden="true" />
                        Paired
                    </span>
                </p>
            ) : (
                <PairingProgressCard
                    pairing={pairing}
                    compact
                    showActions={false}
                />
            )}

            <div className={styles.actions}>
                {state === "idle" ? (
                    <Button
                        type="button"
                        size="md"
                        variant="primary"
                        disabled={disabled}
                        aria-label={`Connect to ${deviceName} with protected pairing`}
                        onClick={onConnect}
                    >
                        <Icon name="shieldCheck" aria-hidden="true" />
                        Connect
                    </Button>
                ) : state === "pending" ? (
                    <Button
                        type="button"
                        size="md"
                        variant="secondary"
                        disabled={cancelling}
                        state={cancelling ? "loading" : "normal"}
                        onClick={() => pairing.run("cancel")}
                    >
                        {cancelling ? "Cancelling…" : "Cancel"}
                    </Button>
                ) : state === "error" ? (
                    <Button
                        type="button"
                        size="md"
                        variant="primary"
                        disabled={disabled}
                        onClick={onConnect}
                    >
                        <Icon name="refresh" aria-hidden="true" />
                        Try again
                    </Button>
                ) : null}
            </div>
        </section>
    );
}
