import { useQuery } from "@tanstack/react-query";
import { useCallback, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { Screen, ScrollViewport } from "@/components/layout";
import { ActionButton, EmptyState, SearchField } from "@/components/shared";
import { Surface } from "@/components/ui";
import { QuickPasteLoadingState } from "@/features/quick-paste/components/QuickPasteLoadingState";
import { QuickPasteRow } from "@/features/quick-paste/components/QuickPasteRow";
import { useItemBody } from "@/hooks/useItemBody";
import {
  QUICK_PASTE_QUERY_KEY,
  useQuickPasteLifecycle,
} from "@/features/quick-paste/hooks/useQuickPasteLifecycle";
import { useQuickPasteSelection } from "@/features/quick-paste/hooks/useQuickPasteSelection";
import {
  QUICK_PASTE_POLL_ACTIVE_MS,
  QUICK_PASTE_POLL_BACKOFF_MS,
} from "@/features/quick-paste/model/quickPastePolling";
import { markedOrigin, markedOrigins } from "@/features/history/model/origin";
import {
  copyItem,
  copyItemAsPlainText,
  listItems,
  openSettingsFromQuickPaste,
  restartService,
  setPinned,
  type Item,
} from "@/lib/ipc";
import { classifyError } from "@/lib/errors";
import { t } from "@/i18n";
import { cn } from "@/lib/cn";
import { rankFuzzy } from "@/lib/fuzzy";
import { kindOf } from "@/lib/format";
import styles from "./QuickPasteScreen.module.css";
const LIMIT = 100;

export function quickPasteSearchLabel(item: Item): string {
  if (item.is_sensitive) return "••••••••";
  const kind = kindOf(item);
  if (kind === "image") return t("quickPaste.row.image");
  if (kind === "file") return t("quickPaste.row.file");
  if (kind === "unknown") return t("quickPaste.row.unsupported");
  if (item.sensitive_finding) return item.sensitive_finding.redacted_preview;
  return item.content ?? "";
}

export function QuickPasteScreen() {
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState("");
  const [pinPendingId, setPinPendingId] = useState<string | null>(null);
  const clearLocalState = useCallback(() => {
    setQuery("");
    setPinPendingId(null);
  }, []);
  const {
    holding,
    previewLinesPopup,
    dismiss,
    dismissOnRootBlur,
    currentCacheGeneration,
    isCacheGenerationCurrent,
  } = useQuickPasteLifecycle({ searchRef, clearLocalState });

  const history = useQuery({
    queryKey: QUICK_PASTE_QUERY_KEY,
    queryFn: () => listItems(LIMIT, null),
    enabled: holding,
    refetchInterval: (request) =>
      request.state.status === "error"
        ? QUICK_PASTE_POLL_BACKOFF_MS
        : QUICK_PASTE_POLL_ACTIVE_MS,
    refetchOnWindowFocus: false,
  });
  const { refetch } = history;

  const items = useMemo(
    () => rankFuzzy(history.data?.items ?? [], query, (item) => [quickPasteSearchLabel(item)]),
    [history.data?.items, query],
  );
  const originMarks = useMemo(
    () => markedOrigins(history.data?.items ?? []),
    [history.data?.items],
  );

  const restart = async () => {
    try {
      await restartService();
      void refetch();
    } catch {
      toast.error(t("quickPaste.toast.restartFailed"), {
        action: { label: t("quickPaste.toast.retry"), onClick: () => void restart() },
      });
    }
  };

  const copyAndDismiss = useCallback(
    async (item: Item, plainText = false) => {
      const generation = currentCacheGeneration();
      try {
        await (plainText ? copyItemAsPlainText(item.id) : copyItem(item.id));
        dismiss();
      } catch (error: unknown) {
        console.error("Quick Paste copy failed", error);
        if (!isCacheGenerationCurrent(generation)) return;
        toast.error(t("quickPaste.toast.copyFailed"), {
          action: {
            label: t("quickPaste.toast.retry"),
            onClick: () => void copyAndDismiss(item, plainText),
          },
        });
      }
    },
    [currentCacheGeneration, dismiss, isCacheGenerationCurrent],
  );

  const changePin = useCallback(
    async (id: string, pinned: boolean) => {
      const generation = currentCacheGeneration();
      setPinPendingId(id);
      try {
        await setPinned(id, pinned);
        if (isCacheGenerationCurrent(generation)) void refetch();
      } catch (error: unknown) {
        console.error("Quick Paste pin failed", error);
        if (isCacheGenerationCurrent(generation)) {
          toast.error(
            t(pinned ? "quickPaste.toast.pinFailed" : "quickPaste.toast.unpinFailed"),
            {
              action: {
                label: t("quickPaste.toast.retry"),
                onClick: () => void changePin(id, pinned),
              },
            },
          );
        }
      } finally {
        if (isCacheGenerationCurrent(generation)) setPinPendingId(null);
      }
    },
    [currentCacheGeneration, isCacheGenerationCurrent, refetch],
  );

  const { selectedId, onKeyDown, selectFromPointer, noteScroll } = useQuickPasteSelection({
    active: holding,
    items,
    query,
    listRef,
    onCopy: copyAndDismiss,
    onDismiss: dismiss,
  });
  const selectedItem = useMemo(
    () => items.find((item) => item.id === selectedId) ?? null,
    [items, selectedId],
  );
  const selectedBody = useItemBody(selectedItem);

  const historyError = history.error ? classifyError(history.error) : null;
  const searching = query.trim().length > 0;

  return (
    <Surface asChild elevation="overlay" border="subtle" radius="lg">
      <Screen
        aria-label={t("quickPaste.title")}
        className={styles.root}
        onKeyDown={onKeyDown}
        onBlur={dismissOnRootBlur}
      >
        <div className={styles.search}>
          <SearchField
            size="compact"
            inputRef={searchRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onClear={() => setQuery("")}
            shortcut=""
            clearLabel={t("quickPaste.search.clear")}
            aria-label={t("quickPaste.search.label")}
            placeholder={t("quickPaste.search.label")}
          />
        </div>

        <ScrollViewport
          ref={listRef}
          role="list"
          onScroll={noteScroll}
          className={cn(styles.list, history.isPending && styles.loadingList)}
        >
          {history.isPending ? (
            <QuickPasteLoadingState
              title={t("quickPaste.loading.title")}
            />
          ) : historyError === "offline" ? (
            <EmptyState
              compact
              icon="plug"
              tone="attention"
              title={t("quickPaste.offline.title")}
              body={t("quickPaste.offline.body")}
              action={{
                label: t("quickPaste.offline.action"),
                icon: "play",
                onClick: () => void restart(),
              }}
            />
          ) : historyError === "not_ready" ? (
            <EmptyState
              busy
              compact
              icon="plug"
              tone="info"
              title={t("quickPaste.starting.title")}
              body={t("quickPaste.starting.body")}
            />
          ) : history.error ? (
            <EmptyState
              compact
              icon="alert"
              tone="danger"
              title={t("quickPaste.failed.title")}
              body={t("quickPaste.failed.body")}
              action={{
                label: t("common.tryAgain"),
                icon: "refresh",
                onClick: () => void refetch(),
              }}
            />
          ) : items.length === 0 ? (
            <EmptyState
              compact
              icon={searching ? "searchX" : "library"}
              title={
                searching
                  ? t("quickPaste.noResults.title", { query })
                  : t("quickPaste.empty.title")
              }
              body={t(searching ? "quickPaste.noResults.body" : "quickPaste.empty.body")}
            />
          ) : (
            items.map((item, index) => (
              <QuickPasteRow
                key={item.id}
                item={item}
                active={selectedId === item.id}
                previewLines={previewLinesPopup}
                shortcut={!searching && index < 9 ? `⌘${index + 1}` : null}
                pinPending={pinPendingId === item.id}
                origin={markedOrigin(item, originMarks)}
                fullContent={selectedId === item.id ? selectedBody.text : null}
                fullContentFailed={selectedId === item.id && selectedBody.failed}
                onSelect={() => selectFromPointer(item.id)}
                onCopy={() => void copyAndDismiss(item)}
                onTogglePin={() => void changePin(item.id, !item.pinned)}
              />
            ))
          )}
        </ScrollViewport>

        <footer className={styles.footer}>
          <div className={styles.footerLayout}>
            <p aria-live="polite" className={styles.count}>
              {history.isPending
                ? ""
                : searching
                  ? t("quickPaste.count.partial", {
                      shown: items.length,
                      total: history.data?.items.length ?? 0,
                    })
                  : (history.data?.total ?? 0) > items.length
                    ? t("quickPaste.count.partial", {
                        shown: items.length,
                        total: history.data?.total ?? 0,
                      })
                    : t("quickPaste.count.all", { count: items.length })}
            </p>
            <ActionButton
              size="compactIcon"
              variant="ghost"
              icon="settings"
              aria-label={t("quickPaste.settings")}
              title={t("quickPaste.settings")}
              className={styles.settingsAction}
              onClick={() =>
                void openSettingsFromQuickPaste().catch(() =>
                  toast.error(t("quickPaste.toast.settingsFailed"), {
                    action: {
                      label: t("quickPaste.toast.retry"),
                      onClick: () => void openSettingsFromQuickPaste(),
                    },
                  }),
                )
              }
            />
          </div>
        </footer>
      </Screen>
    </Surface>
  );
}
