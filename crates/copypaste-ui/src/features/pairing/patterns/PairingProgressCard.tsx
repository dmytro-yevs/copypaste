import { StateView } from "@/components/shared/StateView";
import { Button } from "@/components/ui";
import type { PairingController } from "@/features/pairing/hooks/usePairing";
import {
  pairingClientErrorPresentation,
  pairingPresentation,
} from "@/features/pairing/model/pairingPresentation";
import { useTranslation } from "@/i18n";

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
  const busy = pairing.isChecking || pairing.isPending || (clientError === null && semantics.active);

  if (
    hideIdle &&
    semantics.message_id === "ready" &&
    !pairing.isChecking &&
    !pairing.isPending &&
    pairing.error === null
  ) {
    return null;
  }

  const mode = clientError !== null || semantics.tone === "danger"
    ? "error"
    : semantics.tone === "warning"
      ? "warning"
      : semantics.tone === "success"
        ? "success"
        : semantics.tone === "info" && busy
          ? "loading"
          : "info";
  const live = clientError?.live ?? semantics.live;
  const title = clientError?.title
    ?? (pairing.isChecking
      ? t("devices.pairing.progress.checking")
      : pairing.isPending
        ? t("devices.pairing.progress.opening")
        : presentation.title);
  const description = clientError?.body ?? presentation.detail;
  const actions = showActions ? (
    <>
      {clientError !== null ? (
        <>
          {onClose ? (
            <Button type="button" size="sm" variant="ghost" onClick={onClose}>
              {t("common.close")}
            </Button>
          ) : null}
          {clientError.retry && pairing.canRetry ? (
            <Button
              type="button"
              size="sm"
              variant="secondary"
              icon="refresh"
              disabled={!pairing.protectedPresentationAvailable}
              onClick={pairing.retry}
            >
              {t("common.tryAgain")}
            </Button>
          ) : null}
        </>
      ) : semantics.active ? (
        <>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={pairing.isPending}
            pending={pairing.pendingAction === "cancel"}
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
              icon="shieldCheck"
              disabled={
                pairing.isPending || !pairing.protectedPresentationAvailable
              }
              pending={pairing.pendingAction === "present"}
              onClick={() => pairing.run("present")}
            >
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
          {semantics.retry && pairing.canRetry ? (
            <Button
              type="button"
              size="sm"
              variant="secondary"
              icon="refresh"
              disabled={!pairing.protectedPresentationAvailable}
              onClick={pairing.retry}
            >
              {t("common.tryAgain")}
            </Button>
          ) : null}
        </>
      ) : semantics.message_id === "paired" && onDone ? (
        <Button type="button" size="sm" onClick={onDone}>
          {t("common.done")}
        </Button>
      ) : null}
    </>
  ) : undefined;

  return (
    <StateView
      mode={mode}
      placement={compact ? "inline" : "panel"}
      title={title}
      description={(
        <>
          {description}
          {pairing.presentation === "unavailable" && semantics.active ? (
            <span>
              {t("devices.pairing.presentationUnavailable")}
            </span>
          ) : null}
        </>
      )}
      icon={clientError?.icon ?? semantics.icon}
      actions={actions}
      aria-label="Pairing progress"
      aria-busy={busy || undefined}
      role={live}
      aria-live={live === "alert" ? "assertive" : "polite"}
      aria-atomic="true"
      data-compact={compact || undefined}
      data-tone={clientError?.tone ?? semantics.tone}
      data-state={clientError === null ? semantics.message_id : "client_error"}
    />
  );
}
