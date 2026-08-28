import { InlineNotice } from "@/components/shared";
import { Button, Icon, Surface } from "@/components/ui";
import type { PairingController } from "@/features/pairing/hooks/usePairing";
import {
  pairingClientErrorPresentation,
  pairingPresentation,
} from "@/features/pairing/model/pairingPresentation";
import { useTranslation } from "@/i18n";
import styles from "./PairingProgressCard.module.css";

interface PairingProgressCardProps {
  pairing: PairingController;
  hideIdle?: boolean;
  compact?: boolean;
  showActions?: boolean;
  onDone?: () => void;
  onClose?: () => void;
}

export function PairingProgressCard({
  pairing,
  hideIdle = false,
  compact = false,
  showActions = true,
  onDone,
  onClose,
}: PairingProgressCardProps) {
  const { t } = useTranslation();
  const presentation = pairingPresentation(pairing.ceremony);
  const clientError = pairingClientErrorPresentation(pairing.error);
  const { semantics } = presentation;
  const failed = clientError !== null || (semantics.terminal && semantics.message_id !== "paired");

  if (
    hideIdle &&
    semantics.message_id === "ready" &&
    !pairing.isChecking &&
    !pairing.isPending &&
    pairing.error === null
  ) {
    return null;
  }

  return (
    <Surface asChild elevation="raised" border="subtle" radius="md">
      <section
        aria-label="Pairing progress"
        aria-busy={pairing.isChecking || pairing.isPending || undefined}
        className={styles.root}
        data-compact={compact || undefined}
        data-tone={clientError?.tone ?? semantics.tone}
      >
        <span
          className={styles.iconWell}
          data-state={clientError === null ? semantics.message_id : "client_error"}
          aria-hidden="true"
        >
          {pairing.isChecking || pairing.isPending || (clientError === null && semantics.active) ? (
            <Icon name="spinner" className={styles.spinner} />
          ) : (
            <Icon name={clientError?.icon ?? presentation.icon} />
          )}
        </span>

        <div
          role={clientError?.live ?? semantics.live}
          aria-live={(clientError?.live ?? semantics.live) === "alert" ? "assertive" : "polite"}
          aria-atomic="true"
          className={styles.copy}
        >
          <p className={styles.title}>
            {clientError !== null
              ? clientError.title
              : pairing.isChecking
              ? t("devices.pairing.progress.checking")
              : pairing.isPending
                ? t("devices.pairing.progress.opening")
                : t(presentation.titleKey)}
          </p>
          <p className={styles.body}>
            {clientError !== null
              ? clientError.body
              : presentation.deviceName === undefined
              ? t(presentation.bodyKey)
              : t("devices.pairing.semantic.paired.device", {
                  name: presentation.deviceName,
                })}
          </p>
          {pairing.presentation === "unavailable" && semantics.active ? (
            <InlineNotice tone="warning" icon="alert">
              {t("devices.pairing.presentationUnavailable")}
            </InlineNotice>
          ) : null}
        </div>

        {showActions ? (
          <div className={styles.actions}>
            {semantics.active ? (
              <>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  disabled={pairing.isPending}
                  aria-busy={pairing.isPending || undefined}
                  onClick={() => pairing.run("cancel")}
                >
                  {pairing.pendingAction === "cancel"
                    ? t("devices.pairing.cancelling")
                    : t("common.cancel")}
                </Button>
                {semantics.review_secure ? (
                  <Button
                    type="button"
                    size="sm"
                    variant="secondary"
                    disabled={
                      pairing.isPending || !pairing.protectedPresentationAvailable
                    }
                    aria-busy={pairing.pendingAction === "present" || undefined}
                    onClick={() => pairing.run("present")}
                  >
                    <Icon name="shieldCheck" aria-hidden="true" />
                    {t("devices.pairing.reviewSecure")}
                  </Button>
                ) : null}
              </>
            ) : failed ? (
              <>
                {onClose ? (
                  <Button type="button" size="sm" variant="ghost" onClick={onClose}>
                    {t("common.close")}
                  </Button>
                ) : null}
                {(clientError?.retry ?? semantics.retry) && pairing.canRetry ? (
                  <Button
                    type="button"
                    size="sm"
                    variant="secondary"
                    disabled={!pairing.protectedPresentationAvailable}
                    onClick={pairing.retry}
                  >
                    <Icon name="refresh" aria-hidden="true" />
                    {t("common.tryAgain")}
                  </Button>
                ) : null}
              </>
            ) : semantics.message_id === "paired" && onDone ? (
              <Button type="button" size="sm" onClick={onDone}>
                {t("common.done")}
              </Button>
            ) : null}
          </div>
        ) : null}
      </section>
    </Surface>
  );
}
