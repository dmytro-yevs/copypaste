import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { invalidateHistoryQueries, STATUS_KEY } from "@/hooks/historyRefresh";
import { toFriendly } from "@/lib/errors";
import { POLL_BACKOFF_MS } from "@/lib/scheduling";
import { type ServiceState, serviceState } from "@/lib/ipc";

export const SERVICE_STATE_KEY = ["service-state"] as const;
const MAX_UNHEALTHY_REPROBES = 3;

/** Owns recovery serialization, bounded reprobes and consumer invalidation. */
export function useServiceRecovery() {
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

    return { service, state, operation, matchesApp, matchingRefreshSettled, busy, retryState, recover, refreshConsumers };
}
