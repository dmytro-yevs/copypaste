import { useCallback, useEffect, useRef, useState } from "react";
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
    const [operation, setOperation] = useState<
        "recover" | "refresh" | "state" | null
    >(null);
    const operationRunning = useRef(false);
    const matchingRefreshAttempted = useRef(false);
    const [matchingRefreshSettled, setMatchingRefreshSettled] = useState(false);

    const invalidateConsumers = useCallback(
        () =>
            Promise.all([
                invalidateHistoryQueries(queryClient),
                queryClient.invalidateQueries({ queryKey: STATUS_KEY }),
            ]),
        [queryClient],
    );

    const runExclusive = useCallback(
        async (
            kind: "recover" | "refresh" | "state",
            action: () => Promise<void>,
        ) => {
            if (operationRunning.current) return;
            operationRunning.current = true;
            setOperation(kind);
            try {
                await action();
            } catch (raw) {
                toast.error(toFriendly(raw), { id: "service-recovery" });
            } finally {
                operationRunning.current = false;
                setOperation(null);
            }
        },
        [],
    );

    const invalidateMatchingConsumers = useCallback(async () => {
        setMatchingRefreshSettled(false);
        try {
            await invalidateConsumers();
        } finally {
            setMatchingRefreshSettled(true);
        }
    }, [invalidateConsumers]);

    async function recover(action: () => Promise<ServiceState>) {
        await runExclusive("recover", async () => {
            const next = await action();
            const matches = next.state === "running" && next.matches_app;
            if (matches) matchingRefreshAttempted.current = true;
            queryClient.setQueryData(SERVICE_STATE_KEY, next);
            if (matches) await invalidateMatchingConsumers();
        });
    }

    const refreshConsumers = useCallback(
        () => runExclusive("refresh", invalidateMatchingConsumers),
        [invalidateMatchingConsumers, runExclusive],
    );

    const retryState = useCallback(
        () =>
            runExclusive("state", async () => {
                const result = await service.refetch();
                if (result.error) throw result.error;
            }),
        [runExclusive, service],
    );

    const state = service.data;
    const matchesApp = state?.state === "running" && state.matches_app;

    useEffect(() => {
        if (!matchesApp) {
            matchingRefreshAttempted.current = false;
            setMatchingRefreshSettled(false);
            return;
        }
        if (matchingRefreshAttempted.current) return;
        matchingRefreshAttempted.current = true;
        void refreshConsumers();
    }, [matchesApp, refreshConsumers]);

    const busy = operation !== null || service.isFetching;

    const diagnostics = (
        <Button variant="ghost" size="sm" onClick={onOpenDiagnostics}>
            <Icon name="stethoscope" size="sm" />
            {t("shell.service.diagnostics")}
        </Button>
    );

    if (service.isPending) {
        return (
            <EmptyState
                icon="plug"
                tone="attention"
                busy
                title={t("shell.service.checking.title")}
                body={t("shell.service.checking.body")}
                secondary={diagnostics}
                secondaryPlacement="attached"
            />
        );
    }

    if (service.isError) {
        return (
            <EmptyState
                icon="alert"
                tone="danger"
                busy={busy}
                title={t("shell.service.unhealthy.title")}
                body={toFriendly(service.error)}
                action={{
                    label: t("common.tryAgain"),
                    icon: "refresh",
                    disabled: busy,
                    onClick: () => void retryState(),
                }}
                secondary={diagnostics}
                secondaryPlacement="attached"
            />
        );
    }

    if (state === undefined) {
        return (
            <EmptyState
                icon="alert"
                tone="danger"
                busy={busy}
                title={t("shell.service.unhealthy.title")}
                body={t("shell.service.unhealthy.body")}
                action={{
                    label: t("common.tryAgain"),
                    icon: "refresh",
                    disabled: busy,
                    onClick: () => void retryState(),
                }}
                secondary={diagnostics}
                secondaryPlacement="attached"
            />
        );
    }

    if (state.state === "unhealthy") {
        return (
            <EmptyState
                icon="alert"
                tone="danger"
                busy={busy}
                title={t("shell.service.unhealthy.title")}
                body={t("shell.service.unhealthy.body")}
                action={{
                    label: t("common.tryAgain"),
                    icon: "refresh",
                    disabled: busy,
                    onClick: () => void retryState(),
                }}
                secondary={diagnostics}
                secondaryPlacement="attached"
            />
        );
    }

    if (matchesApp) {
        const refreshing =
            operation === "recover" ||
            operation === "refresh" ||
            !matchingRefreshSettled;
        return (
            <EmptyState
                icon="plug"
                tone="attention"
                busy={refreshing}
                title={t("shell.service.running.title")}
                body={t(
                    refreshing
                        ? "shell.service.running.refreshing"
                        : "shell.service.running.retry",
                )}
                action={
                    refreshing
                        ? undefined
                        : {
                              label: t("common.tryAgain"),
                              icon: "refresh",
                              disabled: busy,
                              onClick: () => void refreshConsumers(),
                          }
                }
                secondary={diagnostics}
                secondaryPlacement="attached"
            />
        );
    }

    if (state.state === "running") {
        return (
            <EmptyState
                icon="alert"
                tone="attention"
                busy={busy}
                title={t("shell.service.outOfDate.title")}
                body={t("shell.service.outOfDate.body")}
                action={{
                    label: t(
                        busy
                            ? "shell.service.outOfDate.restarting"
                            : "shell.service.outOfDate.restart",
                    ),
                    icon: "play",
                    disabled: busy,
                    onClick: () => void recover(restartService),
                }}
                secondary={diagnostics}
                secondaryPlacement="attached"
            />
        );
    }

    if (state.state === "not_installed") {
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
                disabled: busy,
                onClick: () => void recover(startService),
            }}
            secondary={diagnostics}
            secondaryPlacement="attached"
        />
    );
}
