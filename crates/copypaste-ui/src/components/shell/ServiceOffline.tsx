/**
 * A screen with an offer, not an empty list: an empty list says "you have
 * copied nothing" when the truth is "nothing is listening" (bdac.2). Each of
 * ADR-0004's states gets its own copy because the recovery differs.
 *
 * No filesystem path appears in any branch (INV-12), and none of these
 * sentences names a command.
 */
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { PlugZap, RefreshCw, TriangleAlert } from "lucide-react";

import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/EmptyState";
import { useTranslation } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import { type ServiceState, restartService, serviceState, startService } from "@/lib/ipc";

export const SERVICE_KEY = ["service"] as const;

/** Probed rather than polled: the history query's own failure is what brings
 *  this screen up, and a second timer against a service that is down adds
 *  traffic without adding information. */
export function useServiceState() {
  return useQuery<ServiceState>({
    queryKey: SERVICE_KEY,
    queryFn: serviceState,
    retry: false,
  });
}

export function ServiceOffline() {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const service = useServiceState();
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  async function run(action: () => Promise<ServiceState>) {
    setBusy(true);
    setFailure(null);
    try {
      await action();
      await qc.invalidateQueries();
    } catch (raw) {
      // Classified, never rendered raw (INV-12). The bridge's sentences are
      // already user-facing and already path-free.
      setFailure(toFriendly(raw));
    } finally {
      // INV-30: released whatever happened.
      setBusy(false);
    }
  }

  const state = service.data;

  // The upgrade case: the app was replaced and an older service is still
  // holding the socket. ADR-0004 says why a restart can only complete for a
  // service this app started.
  if (state?.state === "running" && !state.matches_app) {
    return (
      <EmptyState
        icon={TriangleAlert}
        busy={busy}
        title={t("shell.service.outOfDate.title")}
        body={t(
          state.ours
            ? "shell.service.outOfDate.ours"
            : "shell.service.outOfDate.theirs",
        )}
        action={{
          label: t(
            busy
              ? "shell.service.outOfDate.restarting"
              : "shell.service.outOfDate.restart",
          ),
          onClick: () => void run(restartService),
        }}
        secondary={<Failure message={failure} />}
      />
    );
  }

  if (state?.state === "not_installed") {
    return (
      <EmptyState
        icon={TriangleAlert}
        title={t("shell.service.notInstalled.title")}
        body={t("shell.service.notInstalled.body")}
        secondary={<Recheck onClick={() => void qc.invalidateQueries()} />}
      />
    );
  }

  return (
    <EmptyState
      icon={PlugZap}
      busy={busy}
      title={t("shell.service.stopped.title")}
      body={t("shell.service.stopped.body")}
      action={{
        label: t(
          busy ? "shell.service.stopped.starting" : "shell.service.stopped.start",
        ),
        onClick: () => void run(startService),
      }}
      secondary={
        <div className="flex flex-col items-center gap-s-2">
          <Failure message={failure} />
          <Recheck onClick={() => void qc.invalidateQueries()} />
        </div>
      }
    />
  );
}

function Failure({ message }: { message: string | null }) {
  if (message === null) return null;
  return (
    <p role="alert" className="text-xs text-err-strong">
      {message}
    </p>
  );
}

function Recheck({ onClick }: { onClick: () => void }) {
  const { t } = useTranslation();
  return (
    <Button variant="ghost" size="sm" onClick={onClick}>
      <RefreshCw aria-hidden="true" />
      {t("shell.service.recheck")}
    </Button>
  );
}
