import {
  CheckCircle2,
  CircleX,
  LoaderCircle,
  QrCode,
  RefreshCw,
  ScanLine,
  ShieldCheck,
  TriangleAlert,
} from "lucide-react";
import { useEffect } from "react";

import { Button } from "@/components/ui/button";
import { usePairing } from "@/hooks/usePairing";
import { useTranslation } from "@/i18n";
import { classifyError, friendlyError } from "@/lib/errors";
import type { PairingCeremony, PairingState } from "@/lib/ipc";

interface PairingPanelProps {
  disabled: boolean;
  hideIntro?: boolean;
  onConfirmed?: () => void;
}

function stateCopy(
  state: PairingState,
  ceremony: PairingCeremony | undefined,
  t: TFunction,
) {
  switch (state) {
    case "waiting_for_peer":
      return {
        title: t("devices.pairing.state.waiting.title"),
        body: t("devices.pairing.state.waiting.body"),
      };
    case "handshaking":
      return {
        title: t("devices.pairing.state.handshaking.title"),
        body: t("devices.pairing.state.handshaking.body"),
      };
    case "awaiting_confirmation":
      return {
        title: t("devices.pairing.state.confirm.title"),
        body: t("devices.pairing.state.confirm.body"),
      };
    case "confirmed":
      return {
        title: t("devices.pairing.state.confirmed.title"),
        body: ceremony?.known_device
          ? t("devices.pairing.state.confirmed.device", {
              name: ceremony.known_device.name,
            })
          : t("devices.pairing.state.confirmed.body"),
      };
    case "rejected":
      return {
        title: t("devices.pairing.state.rejected.title"),
        body: t("devices.pairing.state.rejected.body"),
      };
    case "cancelled":
      return {
        title: t("devices.pairing.state.cancelled.title"),
        body: t("devices.pairing.state.cancelled.body"),
      };
    case "timed_out":
      return {
        title: t("devices.pairing.state.timedOut.title"),
        body: t("devices.pairing.state.timedOut.body"),
      };
    case "failed":
      return {
        title: t("devices.pairing.state.failed.title"),
        body: ceremony?.error
          ? friendlyError(classifyError(ceremony.error))
          : t("devices.pairing.state.failed.body"),
      };
    case "idle":
      return {
        title: t("devices.pairing.state.idle.title"),
        body: t("devices.pairing.state.idle.body"),
      };
  }
}

export function PairingPanel({ disabled, hideIntro, onConfirmed }: PairingPanelProps) {
  const { t } = useTranslation();
  const pairing = usePairing();
  const state = pairing.ceremony?.state ?? "idle";
  const active =
    state === "waiting_for_peer" ||
    state === "handshaking" ||
    state === "awaiting_confirmation";
  const terminal =
    state === "rejected" ||
    state === "cancelled" ||
    state === "timed_out" ||
    state === "failed";
  const copy = stateCopy(state, pairing.ceremony, t);
  const pending = pairing.isPending ? pairing.pendingAction : undefined;
  const unavailable =
    pairing.presentation === "unavailable" &&
    (state !== "idle" || pairing.lastAttempt !== null);

  useEffect(() => {
    if (state === "confirmed") onConfirmed?.();
  }, [onConfirmed, state]);

  return (
    <section
      aria-labelledby={hideIntro ? undefined : "pair-device-heading"}
      aria-label={hideIntro ? t("devices.pairing.heading") : undefined}
      aria-busy={pairing.isPending || undefined}
      className="flex flex-col gap-s-2"
    >
      <div className="flex flex-wrap items-baseline justify-between gap-s-2">
        {hideIntro ? null : (
          <div>
            <h2 id="pair-device-heading" className="text-sm font-semibold">
              {t("devices.pairing.heading")}
            </h2>
            <p className="mt-0.5 text-xs text-muted-foreground">
              {t("devices.pairing.description")}
            </p>
          </div>
        )}
        <div className="flex flex-wrap gap-s-2">
          <Button
            size="sm"
            variant="outline"
            disabled={disabled || active || pairing.isPending}
            title={
              disabled
                ? t("devices.cap.hint")
                : t("devices.pairing.createHint")
            }
            onClick={() => pairing.run("create")}
          >
            {pending === "create" ? (
              <LoaderCircle
                className="animate-spin motion-reduce:animate-none"
                aria-hidden="true"
              />
            ) : (
              <QrCode aria-hidden="true" />
            )}
            {t("devices.pairing.create")}
          </Button>
          <Button
            size="sm"
            disabled={disabled || active || pairing.isPending}
            title={
              disabled ? t("devices.cap.hint") : t("devices.pairing.joinHint")
            }
            onClick={() => pairing.run("join")}
          >
            {pending === "join" ? (
              <LoaderCircle
                className="animate-spin motion-reduce:animate-none"
                aria-hidden="true"
              />
            ) : (
              <ScanLine aria-hidden="true" />
            )}
            {t("devices.pairing.join")}
          </Button>
        </div>
      </div>

      <div className="rounded-xl border border-border bg-card p-s-3 shadow-sm">
        {pairing.isChecking ? (
          <div
            role="status"
            aria-live="polite"
            className="flex items-center gap-s-3"
          >
            <LoaderCircle
              className="animate-spin text-muted-foreground motion-reduce:animate-none"
              aria-hidden="true"
            />
            <p className="text-sm text-muted-foreground">
              {t("devices.pairing.checking")}
            </p>
          </div>
        ) : pairing.error ? (
          <div role="alert" className="flex flex-wrap items-center gap-s-3">
            <TriangleAlert className="text-err-strong" aria-hidden="true" />
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium">
                {t("devices.pairing.state.failed.title")}
              </p>
              <p className="mt-0.5 text-xs text-muted-foreground">
                {friendlyError(classifyError(pairing.error))}
              </p>
            </div>
            <div className="flex flex-wrap gap-s-2">
              {active && (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => pairing.run("cancel")}
                >
                  {t("devices.pairing.cancel")}
                </Button>
              )}
              <Button size="sm" variant="outline" onClick={pairing.retry}>
                <RefreshCw aria-hidden="true" />
                {t("common.tryAgain")}
              </Button>
            </div>
          </div>
        ) : (
          <div
            role={state === "failed" ? "alert" : "status"}
            aria-live="polite"
            aria-atomic="true"
            className="flex flex-wrap items-start gap-s-3"
          >
            <span
              className="flex size-[var(--sz-tile)] shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground"
              aria-hidden="true"
            >
              {state === "confirmed" ? (
                <CheckCircle2 className="text-ok-strong" size={18} />
              ) : terminal ? (
                <CircleX
                  className={
                    state === "timed_out" || state === "failed"
                      ? "text-err-strong"
                      : undefined
                  }
                  size={18}
                />
              ) : active ? (
                <LoaderCircle
                  className="animate-spin motion-reduce:animate-none"
                  size={18}
                />
              ) : (
                <ShieldCheck size={18} />
              )}
            </span>
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium">{copy.title}</p>
              <p className="mt-0.5 text-xs text-muted-foreground">{copy.body}</p>
              {unavailable && (
                <p className="mt-s-2 text-xs text-warn-strong">
                  {t(
                    state === "idle" && pairing.lastAttempt === "join"
                      ? "devices.pairing.scanCancelled"
                      : "devices.pairing.presentationUnavailable",
                  )}
                </p>
              )}
              {pairing.decisionSubmitted !== null && (
                <p className="mt-s-2 text-xs text-info-strong">
                  {t("devices.pairing.decisionSubmitted")}
                </p>
              )}
            </div>

            {active && (
              <div className="flex w-full flex-wrap justify-end gap-s-2 border-t border-divider pt-s-3 sm:w-auto sm:border-0 sm:pt-0">
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={pairing.isPending}
                  onClick={() => pairing.run("cancel")}
                >
                  {t("devices.pairing.cancel")}
                </Button>
                {state !== "waiting_for_peer" && (
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={pairing.isPending}
                    onClick={() => pairing.run("present")}
                  >
                    <ShieldCheck aria-hidden="true" />
                    {pending === "present"
                      ? t("devices.pairing.presenting")
                      : t("devices.pairing.present")}
                  </Button>
                )}
                {state === "awaiting_confirmation" &&
                  pairing.decisionSubmitted === null && (
                    <>
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={pairing.isPending}
                        aria-label={t("devices.pairing.rejectLabel")}
                        onClick={() => pairing.run("reject")}
                      >
                        {pending === "reject"
                          ? t("devices.pairing.rejecting")
                          : t("devices.pairing.reject")}
                      </Button>
                      <Button
                        size="sm"
                        disabled={pairing.isPending}
                        aria-label={t("devices.pairing.confirmLabel")}
                        onClick={() => pairing.run("confirm")}
                      >
                        {pending === "confirm"
                          ? t("devices.pairing.confirming")
                          : t("devices.pairing.confirm")}
                      </Button>
                    </>
                  )}
              </div>
            )}

            {terminal && pairing.canRetry && (
              <Button size="sm" variant="outline" onClick={pairing.retry}>
                <RefreshCw aria-hidden="true" />
                {t("common.tryAgain")}
              </Button>
            )}
          </div>
        )}
      </div>
    </section>
  );
}
