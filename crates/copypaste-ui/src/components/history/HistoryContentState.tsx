/**
 * State resolution follows manifest 06 §3.1.11, with one adjustment: an error
 * only replaces the list when there is nothing else to show. A background poll
 * that fails while 200 rows are on screen must not throw those rows away — the
 * banner and the status chip say the service went away, and the rows stay
 * readable.
 */
import type { ComponentProps } from "react";
import { Archive, CircleAlert, Inbox, KeyRound, Lock, Search } from "lucide-react";

import { EmptyState } from "@/components/EmptyState";
import { HistoryList } from "@/components/history/HistoryList";
import { ServiceOffline } from "@/components/shell/ServiceOffline";
import { useTranslation } from "@/i18n";
import { type ErrorKind, friendlyError } from "@/lib/errors";

interface HistoryContentStateProps {
  loading: boolean;
  errorKind: ErrorKind | null;
  errorRetryable: boolean;
  searching: boolean;
  filtered: boolean;
  privateMode: boolean;
  query: string;
  hasMore: boolean;
  onLoadMore: () => void;
  onRetry: () => void;
  list: ComponentProps<typeof HistoryList>;
}

export function HistoryContentState({
  loading,
  errorKind,
  errorRetryable,
  searching,
  filtered,
  privateMode,
  query,
  hasMore,
  onLoadMore,
  onRetry,
  list,
}: HistoryContentStateProps) {
  const { t } = useTranslation();

  if (loading) {
    return (
      <EmptyState
        busy
        title={t("history.empty.loading.title")}
        body={t("history.empty.loading.body")}
      />
    );
  }

  if (list.items.length > 0) return <HistoryList {...list} />;

  switch (errorKind) {
    case "legacy_database":
      // No action, deliberately. The recovery is a human decision made outside
      // this app, and the only button that would fit here — erase and start
      // over — does not exist yet (backlog B-11). Offering a **Try again**
      // that can never succeed is the defect this replaces.
      return (
        <EmptyState
          icon={Archive}
          title={t("history.empty.legacy.title")}
          body={t("history.empty.legacy.body")}
        />
      );
    case "key_unusable":
      // Also no action, and for a harder reason: there is genuinely nothing to
      // offer. Saying so is better than a control that pretends.
      return (
        <EmptyState
          icon={KeyRound}
          title={t("history.empty.keyUnusable.title")}
          body={t("history.empty.keyUnusable.body")}
        />
      );
    case "key_locked":
      // The one that *is* worth retrying, and the reason the two key-store
      // failures are separate codes at all.
      return (
        <EmptyState
          icon={Lock}
          title={t("history.empty.keyLocked.title")}
          body={t("history.empty.keyLocked.body")}
          action={
            errorRetryable
              ? { label: t("common.tryAgain"), onClick: onRetry }
              : undefined
          }
        />
      );
    case "offline":
      return <ServiceOffline />;
    case "not_ready":
      return (
        <EmptyState
          busy
          title={t("history.empty.starting.title")}
          body={friendlyError("not_ready")}
        />
      );
    case null:
      break;
    default:
      return (
        <EmptyState
          icon={CircleAlert}
          title={t("history.empty.failed.title")}
          body={friendlyError(errorKind)}
          action={
            errorRetryable
              ? { label: t("common.tryAgain"), onClick: onRetry }
              : undefined
          }
        />
      );
  }

  if (privateMode) {
    return (
      <EmptyState
        icon={Lock}
        title="Private mode is on"
        body="Clipboard is not recorded while private mode is active."
      />
    );
  }

  if (filtered) {
    return (
      <EmptyState
        icon={Search}
        title={
          searching
            ? t("history.empty.noResults", { query })
            : t("history.empty.noMatch")
        }
        body={t("history.empty.filteredBody")}
        action={
          hasMore
            ? { label: t("history.empty.loadMore"), onClick: onLoadMore }
            : undefined
        }
      />
    );
  }

  return (
    <EmptyState
      icon={Inbox}
      title={t("history.empty.none.title")}
      body={t("history.empty.none.body")}
    />
  );
}
