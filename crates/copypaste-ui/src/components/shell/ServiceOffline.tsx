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
import { AppWindow, Play, PlugZap, Stethoscope, TriangleAlert } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/EmptyState";
import { useTranslation } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import {
  type ServiceState,
  hasBridge,
  restartService,
  serviceState,
  startService,
} from "@/lib/ipc";
import { useUi } from "@/store/ui";

export const SERVICE_KEY = ["service"] as const;

/** Probed rather than polled: the history query's own failure is what brings
 *  this screen up, and a second timer against a service that is down adds
 *  traffic without adding information. */
export function useServiceState(enabled = true) {
  return useQuery<ServiceState>({
    queryKey: SERVICE_KEY,
    queryFn: serviceState,
    enabled,
    retry: false,
  });
}

export function ServiceOffline() {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const bridge = hasBridge();
  const service = useServiceState(bridge);
  const setView = useUi((state) => state.setView);
  const setSettingsTab = useUi((state) => state.setSettingsTab);
  const [busy, setBusy] = useState(false);

  // The Vite URL is useful for styling the React app, but it is not the
  // CopyPaste application: only Tauri injects the native command bridge.
  // Calling the normal recovery actions here can never reach the daemon and
  // incorrectly tells a developer that their background service is broken.
  if (!bridge) {
    return (
      <EmptyState
        icon={AppWindow}
        tone="info"
        title={t("shell.service.webPreview.title")}
        body={t("shell.service.webPreview.body")}
      />
    );
  }

  async function run(action: () => Promise<ServiceState>) {
    setBusy(true);
    try {
      await action();
      await qc.invalidateQueries();
    } catch (raw) {
      // A failed retry does not change the blocking state already described by
      // this screen. Report it as a transient, path-safe bottom notification.
      toast.error(toFriendly(raw), { id: "service-recovery" });
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
        body={t("shell.service.outOfDate.body")}
        action={
          state.ours
            ? {
                label: t(
                  busy
                    ? "shell.service.outOfDate.restarting"
                    : "shell.service.outOfDate.restart",
                ),
                icon: Play,
                onClick: () => void run(restartService),
              }
            : undefined
        }
        secondary={
          <DiagnosticsLink
            onClick={() => {
              setSettingsTab("diagnostics");
              setView("settings");
            }}
          />
        }
        secondaryPlacement="attached"
      />
    );
  }

  if (state?.state === "not_installed") {
    return (
      <EmptyState
        icon={TriangleAlert}
        title={t("shell.service.notInstalled.title")}
        body={t("shell.service.notInstalled.body")}
        secondary={
          <DiagnosticsLink
            onClick={() => {
              setSettingsTab("diagnostics");
              setView("settings");
            }}
          />
        }
        secondaryPlacement="attached"
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
          icon: Play,
          onClick: () => void run(startService),
      }}
        secondary={
          <DiagnosticsLink
          onClick={() => {
            setSettingsTab("diagnostics");
            setView("settings");
          }}
          />
        }
        secondaryPlacement="attached"
      />
  );
}

function DiagnosticsLink({ onClick }: { onClick: () => void }) {
  const { t } = useTranslation();
  return (
    <Button variant="link" size="sm" onClick={onClick}>
      <Stethoscope aria-hidden="true" />
      {t("shell.service.diagnostics")}
    </Button>
  );
}
