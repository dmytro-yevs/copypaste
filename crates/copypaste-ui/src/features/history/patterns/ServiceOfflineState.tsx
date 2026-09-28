import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { StateView, type StateMode } from "@/components/shared/StateView";
import { Button, Icon } from "@/components/ui";
import { useTranslation } from "@/i18n";
import { invalidateHistoryQueries, STATUS_KEY } from "@/hooks/historyRefresh";
import { toFriendly } from "@/lib/errors";
import { POLL_BACKOFF_MS } from "@/lib/scheduling";
import {
    type ServiceState,
    restartService,
    serviceState,
    startService,
} from "@/lib/ipc";

export const SERVICE_STATE_KEY = ["service-state"] as const;
const MAX_UNHEALTHY_REPROBES = 3;

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
    const unhealthyReprobes = useRef(0);
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
    [runExclusive, service.refetch],
    );

    const state = service.data;
    const matchesApp = state?.state === "running" && state.matches_app;

    useEffect(() => {
        if (state?.state !== "unhealthy") {
            unhealthyReprobes.current = 0;
            return;
        }
        let cancelled = false;
        let timer: number | undefined;
        const probe = async () => {
            unhealthyReprobes.current += 1;
            await retryState();
            if (cancelled || unhealthyReprobes.current >= MAX_UNHEALTHY_REPROBES) return;
            timer = window.setTimeout(probe, POLL_BACKOFF_MS);
        };
        timer = window.setTimeout(probe, POLL_BACKOFF_MS);
        return () => {
            cancelled = true;
            if (timer !== undefined) window.clearTimeout(timer);
        };
    }, [retryState, state?.state]);

    useEffect(() => {
        if (!matchesApp) {
            matchingRefreshAttempted.current = false;
            setMatchingRefreshSettled(false);
            return;
        }
        if (operation !== null) return;
        if (matchingRefreshAttempted.current) return;
        matchingRefreshAttempted.current = true;
        void refreshConsumers();
    }, [matchesApp, operation, refreshConsumers]);

    const busy = operation !== null || service.isFetching;

    const diagnostics = (
        <Button variant="ghost" size="sm" onClick={onOpenDiagnostics}>
            <Icon name="stethoscope" size="sm" />
            {t("shell.service.diagnostics")}
        </Button>
    );
    const retryAction = (
        <Button size="sm" disabled={busy} onClick={() => void retryState()}>
            <Icon name="refresh" size="sm" />
            {t("common.tryAgain")}
        </Button>
    );
    const view = (mode: StateMode, title: string, description: string, action?: ReactNode) => (
        <StateView
            mode={busy ? "loading" : mode}
            placement="screen"
            title={title}
            description={description}
            actions={<>{action}{diagnostics}</>}
            aria-busy={busy || undefined}
        />
    );

    if (service.isPending) {
        return view("loading", t("shell.service.checking.title"), t("shell.service.checking.body"));
    }

    if (service.isError) {
        return view("error", t("shell.service.unhealthy.title"), toFriendly(service.error), retryAction);
    }

    if (state === undefined || state.state === "unhealthy") {
        return view("error", t("shell.service.unhealthy.title"), t("shell.service.unhealthy.body"), retryAction);
    }

    if (matchesApp) {
        const refreshing = operation === "recover" || operation === "refresh" || !matchingRefreshSettled;
        const action = refreshing ? undefined : (
            <Button size="sm" disabled={busy} onClick={() => void refreshConsumers()}>
                <Icon name="refresh" size="sm" />
                {t("common.tryAgain")}
            </Button>
        );
        return view(
            refreshing ? "loading" : "offline",
            t("shell.service.running.title"),
            t(refreshing ? "shell.service.running.refreshing" : "shell.service.running.retry"),
            action,
        );
    }

    if (state.state === "running") {
        return view("warning", t("shell.service.outOfDate.title"), t("shell.service.outOfDate.body"), (
            <Button size="sm" disabled={busy} onClick={() => void recover(restartService)}>
                <Icon name="play" size="sm" />
                {t(busy ? "shell.service.outOfDate.restarting" : "shell.service.outOfDate.restart")}
            </Button>
        ));
    }

    if (state.state === "not_installed") {
        return view("warning", t("shell.service.notInstalled.title"), t("shell.service.notInstalled.body"));
    }

    return view("offline", t("shell.service.stopped.title"), t("shell.service.stopped.body"), (
        <Button size="sm" disabled={busy} onClick={() => void recover(startService)}>
            <Icon name="play" size="sm" />
            {t(busy ? "shell.service.stopped.starting" : "shell.service.stopped.start")}
        </Button>
    ));
}
