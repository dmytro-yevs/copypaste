import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { EmptyState } from "@/components/shared";
import { Button, Icon } from "@/components/ui";
import { useTranslation } from "@/i18n";
import { invalidateHistoryQueries, STATUS_KEY } from "@/hooks/historyRefresh";
import { toFriendly } from "@/lib/errors";
import {
    type ServiceState,
    restartService,
    serviceState,
    startService,
} from "@/lib/ipc";

export const SERVICE_STATE_KEY = ["service-state"] as const;

interface ServiceOfflineStateProps {
    onOpenDiagnostics: () => void;
}

export function ServiceOfflineState({
    onOpenDiagnostics,
}: ServiceOfflineStateProps) {
    const { t } = useTranslation();
    const queryClient = useQueryClient();
    const service = useQuery<ServiceState>({
        queryKey: SERVICE_STATE_KEY,
        queryFn: serviceState,
        retry: false,
    });
    const [busy, setBusy] = useState(false);

    async function recover(action: () => Promise<ServiceState>) {
        setBusy(true);
        try {
            const state = await action();
            queryClient.setQueryData(SERVICE_STATE_KEY, state);
            await Promise.all([
                invalidateHistoryQueries(queryClient),
                queryClient.invalidateQueries({ queryKey: STATUS_KEY }),
            ]);
        } catch (raw) {
            toast.error(toFriendly(raw), { id: "service-recovery" });
        } finally {
            setBusy(false);
        }
    }

    const diagnostics = (
        <Button variant="ghost" size="sm" onClick={onOpenDiagnostics}>
            <Icon name="stethoscope" size="sm" />
            {t("shell.service.diagnostics")}
        </Button>
    );
    const state = service.data;

    if (state?.state === "running" && !state.matches_app) {
        return (
            <EmptyState
                icon="alert"
                tone="attention"
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
                              icon: "play",
                              onClick: () => void recover(restartService),
                          }
                        : undefined
                }
                secondary={diagnostics}
                secondaryPlacement="attached"
            />
        );
    }

    if (state?.state === "not_installed") {
        return (
            <EmptyState
                icon="alert"
                tone="attention"
                title={t("shell.service.notInstalled.title")}
                body={t("shell.service.notInstalled.body")}
                secondary={diagnostics}
                secondaryPlacement="attached"
            />
        );
    }

    return (
        <EmptyState
            icon="plug"
            tone="attention"
            busy={busy}
            title={t("shell.service.stopped.title")}
            body={t("shell.service.stopped.body")}
            action={{
                label: t(
                    busy
                        ? "shell.service.stopped.starting"
                        : "shell.service.stopped.start",
                ),
                icon: "play",
                onClick: () => void recover(startService),
            }}
            secondary={diagnostics}
            secondaryPlacement="attached"
        />
    );
}
