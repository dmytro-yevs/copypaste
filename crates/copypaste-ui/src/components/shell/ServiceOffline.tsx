/**
 * A screen with an offer, not an empty list: an empty list says "you have
 * copied nothing" when the truth is "nothing is listening" — the same defect as
 * bdac.2, one screen over.
 *
 * ADR-0004 made the offer real. The app owns the background service's lifetime
 * and ships it inside its own bundle, so the button starts it rather than
 * telling the user to open a terminal. The four situations the ADR enumerates
 * each get their own copy, because the recovery differs:
 *
 * | state | what the user can do |
 * |---|---|
 * | `stopped` | press Start |
 * | `not_installed` | nothing here; the build has no service to start |
 * | `running` with a different version | restart it, if this app started it |
 * | `unhealthy` | wait, or restart |
 *
 * No filesystem path appears in any branch (INV-12), and none of these
 * sentences names a command.
 */
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { PlugZap, RefreshCw, TriangleAlert } from "lucide-react";

import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/EmptyState";
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

  // A version mismatch is the upgrade case: the app was replaced and an older
  // service is still holding the socket. Restart is the fix, and ADR-0004
  // explains why it can only complete for a service this app started.
  if (state?.state === "running" && !state.matches_app) {
    return (
      <EmptyState
        icon={TriangleAlert}
        busy={busy}
        title="The background service is out of date"
        body={
          state.ours
            ? "A different version of the background service is running. Restart it to pick up this update."
            : "A different version of the background service is running, and it was started by something else — CopyPaste can't stop it. Quit it, then restart."
        }
        action={{
          label: busy ? "Restarting…" : "Restart the service",
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
        title="This build has no background service"
        body="CopyPaste records your clipboard from a small background service, and this build doesn't include one. Install CopyPaste from the official package to get it."
        secondary={<Recheck onClick={() => void qc.invalidateQueries()} />}
      />
    );
  }

  return (
    <EmptyState
      icon={PlugZap}
      busy={busy}
      title="The clipboard service isn't running"
      body="CopyPaste records your clipboard from a small background service. It isn't running, so nothing is being saved right now."
      action={{
        label: busy ? "Starting…" : "Start the service",
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
  return (
    <Button variant="ghost" size="sm" onClick={onClick}>
      <RefreshCw aria-hidden="true" />
      Check again
    </Button>
  );
}
