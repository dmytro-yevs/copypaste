/**
 * State resolution follows manifest 06 §3.1.11, with one adjustment: an error
 * only replaces the list when there is nothing else to show. A background poll
 * that fails while 200 rows are on screen must not throw those rows away — the
 * banner and the status chip say the service went away, and the rows stay
 * readable.
 */
import type { ComponentProps } from "react";
import {
  CircleAlert,
  ClipboardX,
  ChevronDown,
  Inbox,
  KeyRound,
  Lock,
  RefreshCw,
  Search,
  SlidersHorizontal,
} from "lucide-react";

import { EmptyState } from "@/components/EmptyState";
import { RecoveryReportActions } from "@/components/diagnostics/SupportReportActions";
import { HistoryList } from "@/components/history/HistoryList";
import { ServiceOffline } from "@/components/shell/ServiceOffline";
import { useTranslation } from "@/i18n";
import { type ErrorKind, friendlyError } from "@/lib/errors";

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
  list: ComponentProps<typeof HistoryList>;
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
  list,
}: HistoryContentStateProps) {
  const { t } = useTranslation();

  if (loading) {
    return (
        <EmptyState
          busy
          tone="info"
          title={t("history.empty.loading.title")}
        body={t("history.empty.loading.body")}
      />
    );
  }

  if (list.items.length > 0) return <HistoryList {...list} />;

  switch (errorKind) {
    case "key_unusable":
      return (
        <EmptyState
          icon={KeyRound}
          tone="danger"
          title={t("history.empty.keyUnusable.title")}
          body={t("history.empty.keyUnusable.body")}
          action={{ label: t("common.tryAgain"), icon: RefreshCw, onClick: onRetry }}
          secondary={<RecoverySupport />}
        />
      );
    case "key_locked":
      return (
        <EmptyState
          icon={Lock}
          tone="attention"
          title={t("history.empty.keyLocked.title")}
          body={t("history.empty.keyLocked.body")}
          action={{ label: t("common.tryAgain"), icon: RefreshCw, onClick: onRetry }}
          secondary={<RecoverySupport />}
        />
      );
    case "offline":
      return <ServiceOffline />;
    case "not_ready":
      return (
        <EmptyState
          busy
          tone="info"
          title={t("history.empty.starting.title")}
          body={friendlyError("not_ready")}
          action={{ label: t("common.tryAgain"), icon: RefreshCw, onClick: onRetry }}
          secondary={<RecoverySupport />}
        />
      );
    case null:
      break;
    default:
      return (
        <EmptyState
          icon={CircleAlert}
          tone="danger"
          title={t("history.empty.failed.title")}
          body={friendlyError(errorKind)}
          action={{ label: t("common.tryAgain"), icon: RefreshCw, onClick: onRetry }}
          secondary={<RecoverySupport />}
        />
      );
  }

  if (privateMode) {
    return (
      <EmptyState
        icon={Lock}
        tone="private"
        title={t("history.empty.private.title")}
        body={t("history.empty.private.body")}
      />
    );
  }

  if (capturePaused) {
    return (
      <EmptyState
        icon={ClipboardX}
        tone="attention"
        title={t("history.empty.capturePaused.title")}
        body={t("history.empty.capturePaused.body")}
        action={{
          label: t("history.empty.capturePaused.action"),
          icon: SlidersHorizontal,
          onClick: onOpenCapture,
        }}
      />
    );
  }

  if (filtered) {
    return (
      <EmptyState
        icon={Search}
        tone="info"
        title={
          searching
            ? t("history.empty.noResults", { query })
            : t("history.empty.noMatch")
        }
        body={t("history.empty.filteredBody")}
        action={
          hasMore
            ? { label: t("history.empty.loadMore"), icon: ChevronDown, onClick: onLoadMore }
            : undefined
        }
      />
    );
  }

  return (
    <EmptyState
      icon={Inbox}
      tone="neutral"
      title={t("history.empty.none.title")}
      body={t("history.empty.none.body")}
    />
  );
}

function RecoverySupport() {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col items-center gap-s-2">
      <p className="text-xs text-muted-foreground">
        {t("history.empty.failed.support")}
      </p>
      <RecoveryReportActions />
    </div>
  );
}
