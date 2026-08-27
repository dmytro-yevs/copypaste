/**
 * State resolution follows manifest 06 §3.1.11, with one adjustment: an error
 * only replaces the list when there is nothing else to show. A background poll
 * that fails while 200 rows are on screen must not throw those rows away — the
 * banner and the status chip say the service went away, and the rows stay
 * readable.
 */
import type { ComponentProps, ReactNode } from "react";

import { IllustratedErrorState, InlineNotice } from "@/components/shared";
import { Button, Icon, type IconName } from "@/components/ui";
import { HistoryLoadingState } from "@/features/history/patterns/HistoryLoadingState";
import { HistoryList } from "@/features/history/patterns/HistoryList";
import { ServiceOfflineState } from "@/features/history/patterns/ServiceOfflineState";
import { useTranslation } from "@/i18n";
import { type ErrorKind, friendlyError } from "@/lib/errors";
import styles from "./HistoryContentState.module.css";

interface HistoryContentStateProps {
    loading: boolean;
    errorKind: ErrorKind | null;
    searching: boolean;
    filtered: boolean;
    privateMode: boolean;
    capturePaused: boolean;
    query: string;
    hasMore: boolean;
    onLoadMore: () => void;
    onRetry: () => void;
    onOpenCapture: () => void;
    onOpenDiagnostics: () => void;
    list: ComponentProps<typeof HistoryList>;
}

function InlineLibraryState({
    icon,
    title,
    body,
    actions,
}: {
    icon: IconName;
    title: string;
    body: string;
    actions?: ReactNode;
}) {
    return (
        <section className={styles.inlineState} role="status">
            <span className={styles.inlineIcon} aria-hidden="true">
                <Icon name={icon} size="sm" />
            </span>
            <span className={styles.inlineCopy}>
                <strong>{title}</strong>
                <span>{body}</span>
            </span>
            {actions ? <span className={styles.inlineActions}>{actions}</span> : null}
        </section>
    );
}

export function HistoryContentState({
    loading,
    errorKind,
    searching,
    filtered,
    privateMode,
    capturePaused,
    query,
    hasMore,
    onLoadMore,
    onRetry,
    onOpenCapture,
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
            <HistoryLoadingState
                title={t("history.empty.loading.title")}
            />
        );
    }

    if (list.items.length > 0) {
        return (
            <>
                {errorKind === "offline" ? (
                    <InlineNotice
                        role="status"
                        tone="warning"
                        icon="plug"
                        action={
                            <span className={styles.inlineActions}>
                                {retryAction}
                                {diagnosticsAction}
                            </span>
                        }
                    >
                        {t("shell.service.stopped.title")}
                    </InlineNotice>
                ) : null}
                <HistoryList {...list} />
            </>
        );
    }

    switch (errorKind) {
        case "key_unusable":
            return (
                <IllustratedErrorState
                    title={t("history.empty.keyUnusable.title")}
                    body={friendlyError("key_unusable")}
                    actions={diagnosticsAction}
                />
            );
        case "key_locked":
            return (
                <IllustratedErrorState
                    title={t("history.empty.keyLocked.title")}
                    body={friendlyError("key_locked")}
                    actions={<>{retryAction}{diagnosticsAction}</>}
                />
            );
        case "offline":
            return (
                <ServiceOfflineState onOpenDiagnostics={onOpenDiagnostics} />
            );
        case "not_ready":
            return (
                <HistoryLoadingState
                    title={t("history.empty.starting.title")}
                />
            );
        case null:
            break;
        default:
            return (
                <IllustratedErrorState
                    title={t("history.empty.failed.title")}
                    body={friendlyError(errorKind)}
                    actions={<>{retryAction}{diagnosticsAction}</>}
                />
            );
    }

    if (privateMode) {
        return (
            <InlineLibraryState
                icon="lock"
                title={t("history.empty.private.title")}
                body={t("history.empty.private.body")}
            />
        );
    }

    if (capturePaused) {
        return (
            <InlineLibraryState
                icon="library"
                title={t("history.empty.capturePaused.title")}
                body={t("history.empty.capturePaused.body")}
                actions={
                    <Button variant="secondary" size="sm" onClick={onOpenCapture}>
                        <Icon name="sliders" size="sm" />
                        {t("history.empty.capturePaused.action")}
                    </Button>
                }
            />
        );
    }

    if (filtered) {
        return (
            <InlineLibraryState
                icon="search"
                title={
                    searching
                        ? t("history.empty.noResults", { query })
                        : t("history.empty.noMatch")
                }
                body={t("history.empty.filteredBody")}
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
        <InlineLibraryState
            icon="library"
            title={t("history.empty.none.title")}
            body={t("history.empty.none.body")}
        />
    );
}
