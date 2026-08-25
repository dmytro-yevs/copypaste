import { Icon } from "@/components/ui/icon";
import { InlineNotice } from "@/components/shared";
import { Button, Surface } from "@/components/ui";
import type { PairingController } from "@/features/pairing/hooks/usePairing";
import { isPairingActive } from "@/features/pairing/model/pairingSession";
import { classifyError, friendlyError } from "@/lib/errors";
import type { PairingCeremony } from "@/lib/ipc";
import styles from "./PairingProgressCard.module.css";

interface PairingProgressCardProps {
  pairing: PairingController;
  hideIdle?: boolean;
  compact?: boolean;
  showActions?: boolean;
  onDone?: () => void;
  onClose?: () => void;
}

function copyFor(ceremony: PairingCeremony | undefined) {
  switch (ceremony?.state ?? "idle") {
    case "waiting_for_peer":
      return {
        title: "Waiting for the other device…",
        body: "Keep the protected pairing surface open on both devices.",
      };
    case "handshaking":
      return {
        title: "Establishing a private connection…",
        body: "CopyPaste is verifying the pairing request.",
      };
    case "awaiting_confirmation":
      return {
        title: "Confirm on the protected pairing surface",
        body: "Compare the security code there. The code is never shown in this page.",
      };
    case "confirmed":
      return {
        title: "Device paired",
        body: ceremony?.known_device
          ? `${ceremony.known_device.name} is ready to sync.`
          : "The device is ready to sync.",
      };
    case "rejected":
      return {
        title: "Pairing didn’t match",
        body: "Nothing was paired. Start again when both devices are ready.",
      };
    case "cancelled":
      return {
        title: "Pairing cancelled",
        body: "No device was added.",
      };
    case "timed_out":
      return {
        title: "Pairing code expired",
        body: "Start again to create a new protected pairing session.",
      };
    case "failed":
      return {
        title: "Pairing failed",
        body: ceremony?.error
          ? friendlyError(classifyError(ceremony.error))
          : "CopyPaste couldn’t finish pairing.",
      };
    case "idle":
      return {
        title: "Pair another device",
        body: "Pairing codes and security comparisons open in a protected native surface.",
      };
  }
}

export function PairingProgressCard({
  pairing,
  hideIdle = false,
  compact = false,
  showActions = true,
  onDone,
  onClose,
}: PairingProgressCardProps) {
  const state = pairing.ceremony?.state ?? "idle";
  const active = isPairingActive(state);
  const terminal =
    state === "rejected" ||
    state === "cancelled" ||
    state === "timed_out" ||
    state === "failed";
  const copy = copyFor(pairing.ceremony);

  if (hideIdle && state === "idle" && !pairing.isChecking && !pairing.isPending && pairing.error === null) {
    return null;
  }

  const visibleError = pairing.error
    ? friendlyError(classifyError(pairing.error))
    : null;
  const failed = terminal || visibleError !== null;

  return (
    <Surface
      asChild
      elevation="raised"
      border="subtle"
      radius="md"
    >
      <section
        aria-label="Pairing progress"
        aria-busy={pairing.isChecking || pairing.isPending || undefined}
        className={styles.root}
        data-compact={compact || undefined}
      >
        <span className={styles.iconWell} data-state={state} aria-hidden="true">
          {visibleError || state === "failed" ? (
            <Icon name="alert" />
          ) : pairing.isChecking || pairing.isPending || active ? (
            <Icon name="spinner" className={styles.spinner} />
          ) : failed ? (
            <Icon name="close" />
          ) : state === "confirmed" ? (
            <Icon name="checkCircle" />
          ) : (
            <Icon name="shieldCheck" />
          )}
        </span>

        <div
          role={visibleError || state === "failed" ? "alert" : "status"}
          aria-live={visibleError || state === "failed" ? "assertive" : state === "confirmed" ? "polite" : undefined}
          aria-atomic="true"
          className={styles.copy}
        >
          <p className={styles.title}>
            {pairing.isChecking
              ? "Checking pairing status…"
              : pairing.isPending
                ? "Opening protected pairing…"
                : copy.title}
          </p>
          <p className={styles.body}>
            {visibleError ?? copy.body}
          </p>
          {pairing.presentation === "unavailable" && active ? (
            <InlineNotice tone="warning" icon="alert">
              The protected pairing surface is unavailable. Cancel and try again.
            </InlineNotice>
          ) : null}
        </div>

        {showActions ? <div className={styles.actions}>
          {active ? (
            <>
              <Button
                type="button"
                size="sm"
                variant="ghost"
                disabled={pairing.isPending}
                aria-busy={pairing.isPending || undefined}
                onClick={() => pairing.run("cancel")}
              >
                {pairing.pendingAction === "cancel" ? "Cancelling…" : "Cancel"}
              </Button>
              {state === "awaiting_confirmation" ? (
                <Button
                  type="button"
                  size="sm"
                  variant="secondary"
                  disabled={pairing.isPending || !pairing.protectedPresentationAvailable}
                  aria-busy={pairing.pendingAction === "present" || undefined}
                  onClick={() => pairing.run("present")}
                >
                  <Icon name="shieldCheck" aria-hidden="true" />
                  Review securely
                </Button>
              ) : null}
            </>
          ) : failed ? (
            <>
              {onClose && visibleError === null ? (
                <Button type="button" size="sm" variant="ghost" onClick={onClose}>
                  Close
                </Button>
              ) : null}
              {pairing.canRetry ? (
                <Button
                  type="button"
                  size="sm"
                  variant="secondary"
                  disabled={!pairing.protectedPresentationAvailable}
                  onClick={pairing.retry}
                >
                  <Icon name="refresh" aria-hidden="true" />
                  Try again
                </Button>
              ) : null}
            </>
          ) : state === "confirmed" && onDone ? (
            <Button type="button" size="sm" onClick={onDone}>
              Done
            </Button>
          ) : null}
        </div> : null}
      </section>
    </Surface>
  );
}
