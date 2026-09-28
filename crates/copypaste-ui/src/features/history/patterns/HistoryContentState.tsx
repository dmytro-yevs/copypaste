/**
 * State resolution follows manifest 06 §3.1.11, with one adjustment: an error
 * only replaces the list when there is nothing else to show. A background poll
 * that fails while 200 rows are on screen must not throw those rows away — the
 * banner and the status chip say the service went away, and the rows stay
 * readable.
 */
import type { ComponentProps } from "react";

import { StateView } from "@/components/shared/StateView";
import { Button, Icon } from "@/components/ui";
import { HistoryList } from "@/features/history/patterns/HistoryList";
import { ServiceOfflineState } from "@/features/history/patterns/ServiceOfflineState";
import { useTranslation } from "@/i18n";
import { type ErrorKind, friendlyError } from "@/lib/errors";

interface HistoryContentStateProps {
    loading: boolean;
    errorKind: ErrorKind | null;
    searching: boolean;
    filtered: boolean;
    privateMode: boolean;
    query: string;
    hasMore: boolean;
    onLoadMore: () => void;
    onRetry: () => void;
    onOpenDiagnostics: () => void;
    list: ComponentProps<typeof HistoryList>;
}

export function HistoryContentState({
    loading,
    errorKind,
    searching,
    filtered,
    privateMode,
    query,
    hasMore,
    onLoadMore,
    onRetry,
    onOpenDiagnostics,
    list,
}: HistoryContentStateProps) {
    const { t } = useTranslation();
    const diagnosticsAction = (
        <Button variant="ghost" size="sm" onClick={onOpenDiagnostics}>
            <Icon name="stethoscope" size="sm" />
            {t("shell.service.diagnostics")}
        </Button>
    );
    const retryAction = (
        <Button size="sm" onClick={onRetry}>
            <Icon name="refresh" size="sm" />
            {t("common.tryAgain")}
        </Button>
    );

    if (loading) {
        return (
            <StateView mode="loading" placement="panel" title={t("history.empty.loading.title")} />
        );
    }

    if (list.items.length > 0) {
        return (
            <>
                {errorKind === "offline" ? (
                    <StateView
                        mode="offline"
                        placement="inline"
                        title={t("shell.service.stopped.title")}
                        actions={<>{retryAction}{diagnosticsAction}</>}
                    />
                ) : null}
                <HistoryList {...list} />
            </>
        );
    }

    switch (errorKind) {
        case "key_unusable":
            return (
                <StateView mode="error" placement="screen"
                    title={t("history.empty.keyUnusable.title")}
                    description={friendlyError("key_unusable")}
                    actions={diagnosticsAction}
                />
            );
        case "key_locked":
            return (
                <StateView mode="error" placement="screen"
                    title={t("history.empty.keyLocked.title")}
                    description={friendlyError("key_locked")}
                    actions={<>{retryAction}{diagnosticsAction}</>}
                />
            );
        case "offline":
            return (
                <ServiceOfflineState onOpenDiagnostics={onOpenDiagnostics} />
            );
        case "not_ready":
            return (
                <StateView mode="loading" placement="panel" title={t("history.empty.starting.title")} />
            );
        case null:
            break;
        default:
            return (
                <StateView mode="error" placement="screen"
                    title={t("history.empty.failed.title")}
                    description={friendlyError(errorKind)}
                    actions={<>{retryAction}{diagnosticsAction}</>}
                />
            );
    }

    if (privateMode) {
        return (
            <StateView mode="empty" placement="inline"
                icon="lock"
                title={t("history.empty.private.title")}
                description={t("history.empty.private.body")}
            />
        );
    }

    if (filtered) {
        return (
            <StateView mode="empty" placement="inline"
                icon="search"
                title={
                    searching
                        ? t("history.empty.noResults", { query })
                        : t("history.empty.noMatch")
                }
                description={t("history.empty.filteredBody")}
                actions={
                    hasMore
                        ? <Button variant="secondary" size="sm" onClick={onLoadMore}>
                              <Icon name="caretDown" size="sm" />
                              {t("history.empty.loadMore")}
                          </Button>
                        : undefined
                }
            />
        );
    }

    return (
        <StateView mode="empty" placement="inline"
            icon="library"
            title={t("history.empty.none.title")}
            description={t("history.empty.none.body")}
        />
    );
}
