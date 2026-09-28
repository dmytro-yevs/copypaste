import type { ReactNode } from "react";

import { StateView, type StateMode } from "@/components/shared/StateView";
import { Button, Icon } from "@/components/ui";
import { useTranslation } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import { restartService, startService } from "@/lib/ipc";
import { useServiceRecovery } from "./useServiceRecovery";

interface ServiceOfflineStateProps {
    onOpenDiagnostics: () => void;
}

export function ServiceOfflineState({ onOpenDiagnostics }: ServiceOfflineStateProps) {
    const { t } = useTranslation();
    const { service, state, operation, matchesApp, matchingRefreshSettled, busy, retryState, recover, refreshConsumers } = useServiceRecovery();
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
