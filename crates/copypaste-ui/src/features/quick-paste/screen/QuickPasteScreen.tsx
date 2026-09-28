import { useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useCallback, useEffect, useMemo, useRef, useState, type UIEvent } from "react";
import { toast } from "sonner";

import { Screen, ScrollViewport } from "@/components/layout";
import { SearchField } from "@/components/shared";
import { StateView } from "@/components/shared/StateView";
import { Button, Surface } from "@/components/ui";
import { QuickPastePreview } from "@/features/quick-paste/components/QuickPastePreview";
import { QuickPasteRow } from "@/features/quick-paste/components/QuickPasteRow";
import { clipboardCopyPresentation } from "@/features/history/model/clipPresentation";
import { historyOf, useHistory } from "@/hooks/useHistory";
import { clipboardWriteAvailabilityOptions, requireClipboardWriteAvailability } from "@/hooks/useClipboardWriteAvailability";
import { useItemBody } from "@/hooks/useItemBody";
import {
  useQuickPasteLifecycle,
} from "@/features/quick-paste/hooks/useQuickPasteLifecycle";
import { useQuickPasteSelection } from "@/features/quick-paste/hooks/useQuickPasteSelection";
import { quickPastePresentation } from "@/features/quick-paste/model/quickPastePresentation";
import {
  copyItem,
  copyItemAsPlainText,
  openSettingsFromQuickPaste,
  restartService,
  setQuickPastePreview,
  setPinned,
  type Item,
  type QuickPastePreviewLayout,
} from "@/lib/ipc";
import { classifyError, isRetryable } from "@/lib/errors";
import { t } from "@/i18n";
import { cn } from "@/lib/cn";
import { acceleratorLabel } from "@/lib/accelerator";
import { rankFuzzy } from "@/lib/fuzzy";
import { markedOrigin, markedOrigins } from "@/lib/itemOrigin";
import styles from "./QuickPasteScreen.module.css";

const QUICK_PASTE_ROW_ESTIMATE_PX = 40;
const QUICK_PASTE_OVERSCAN_ROWS = 5;

export function QuickPasteScreen() {
  const queryClient = useQueryClient();
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState("");
  const [pinPendingId, setPinPendingId] = useState<string | null>(null);
  const copyInFlight = useRef(false);
  const [copyPending, setCopyPending] = useState(false);
  const [previewLayout, setPreviewLayout] = useState<QuickPastePreviewLayout | null>(null);
  const previewOpen = useRef(false);
  const previewRequestEpoch = useRef(0);
  const clearLocalState = useCallback(() => {
    setQuery("");
    setPinPendingId(null);
  }, []);
  const {
    holding,
    dismiss,
    dismissOnRootBlur,
    currentCacheGeneration,
    isCacheGenerationCurrent,
  } = useQuickPasteLifecycle({ searchRef, clearLocalState });

  const history = useHistory(query, false, holding);
  const loadedHistory = historyOf(history.data);
  const { refetch } = history;

  const items = useMemo(
    () => rankFuzzy(loadedHistory.items, query, (item) => [quickPastePresentation(item).searchLabel]),
    [loadedHistory.items, query],
  );
  const originMarks = useMemo(
    () => markedOrigins(loadedHistory.items),
    [loadedHistory.items],
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
      if (copyInFlight.current) return;
      copyInFlight.current = true;
      setCopyPending(true);
      const generation = currentCacheGeneration();
      try {
        let availability;
        try {
          availability = await requireClipboardWriteAvailability(
            queryClient,
            item.content_type,
            plainText ? "plain_text" : "original",
          );
        } catch {
          if (isCacheGenerationCurrent(generation)) {
            toast.error(clipboardCopyPresentation({ status: "failed" }).reason);
          }
          return;
        }
        if (availability !== "available") {
          if (isCacheGenerationCurrent(generation)) {
            toast.error(clipboardCopyPresentation({ status: "resolved", availability }).reason);
          }
          return;
        }
        if (!isCacheGenerationCurrent(generation)) return;
        await (plainText ? copyItemAsPlainText(item.id) : copyItem(item.id));
        if (isCacheGenerationCurrent(generation)) dismiss();
      } catch (error: unknown) {
        console.error("Quick Paste copy failed", error);
        if (!isCacheGenerationCurrent(generation)) return;
        toast.error(
          t("quickPaste.toast.copyFailed"),
          isRetryable(error)
            ? {
                action: {
                  label: t("quickPaste.toast.retry"),
                  onClick: () => void copyAndDismiss(item, plainText),
                },
              }
            : undefined,
        );
      } finally {
        copyInFlight.current = false;
        setCopyPending(false);
      }
    },
    [currentCacheGeneration, dismiss, isCacheGenerationCurrent, queryClient],
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

  const canCopy = useCallback(
    (item: Item, plainText: boolean) =>
      queryClient.getQueryData(
        clipboardWriteAvailabilityOptions(item.content_type, plainText ? "plain_text" : "original").queryKey,
      ) === "available",
    [queryClient],
  );

  const loadMore = useCallback(() => {
    if (!history.hasNextPage || history.isFetchingNextPage) return;
    void history.fetchNextPage();
  }, [history.fetchNextPage, history.hasNextPage, history.isFetchingNextPage]);
  const virtualizer = useVirtualizer({
    count: items.length + (history.hasNextPage ? 1 : 0),
    getScrollElement: () => listRef.current,
    estimateSize: () => QUICK_PASTE_ROW_ESTIMATE_PX,
    getItemKey: (index) => items[index]?.id ?? "loading-more",
    overscan: QUICK_PASTE_OVERSCAN_ROWS,
    useFlushSync: false,
  });
  const virtualRows = virtualizer.getVirtualItems();
  const scrollToItemIndex = useCallback(
    (index: number) => virtualizer.scrollToIndex(index, { align: "auto" }),
    [virtualizer],
  );
  const { selectedId, onKeyDown, selectFromPointer, selectFromKeyboard, noteScroll } = useQuickPasteSelection({
    active: holding,
    items,
    query,
    scrollToItemIndex,
    hasMore: history.hasNextPage,
    onLoadMore: loadMore,
    sessionKey: holding ? currentCacheGeneration() : -1,
    canCopy,
    copyPending,
    onCopy: copyAndDismiss,
    onDismiss: dismiss,
  });
  const onScroll = useCallback(
    (event: UIEvent<HTMLDivElement>) => {
      noteScroll();
      const element = event.currentTarget;
      if (element.scrollHeight - element.scrollTop - element.clientHeight < QUICK_PASTE_ROW_ESTIMATE_PX * 3) {
        loadMore();
      }
    },
    [loadMore, noteScroll],
  );
  const selectedItem = useMemo(
    () => items.find((item) => item.id === selectedId) ?? null,
    [items, selectedId],
  );
  const selectedBody = useItemBody(selectedItem);
  const previewWanted = holding && selectedItem !== null && !selectedItem.is_sensitive;
  const releasePreview = useCallback(() => {
    previewRequestEpoch.current += 1;
    if (!previewOpen.current) return;
    previewOpen.current = false;
    setPreviewLayout(null);
    void setQuickPastePreview(false).catch(() => {});
  }, []);

  useEffect(() => {
    if (!previewWanted) {
      releasePreview();
      return;
    }
    if (previewOpen.current) return;
    const requestEpoch = ++previewRequestEpoch.current;
    previewOpen.current = true;
    void setQuickPastePreview(true)
      .then((layout) => {
        if (requestEpoch !== previewRequestEpoch.current) return;
        if (layout.side === "hidden" || layout.width <= 0) {
          previewOpen.current = false;
          setPreviewLayout(null);
          return;
        }
        setPreviewLayout(layout);
      })
      .catch(() => {
        previewOpen.current = false;
        setPreviewLayout(null);
      });
  }, [previewWanted, releasePreview]);

  useEffect(() => releasePreview, [releasePreview]);

  const historyError = history.error ? classifyError(history.error) : null;
  const searching = query.trim().length > 0;

  return (
    <main className={styles.frame} data-preview-side={previewLayout?.side ?? "hidden"}>
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
          aria-busy={history.isFetchingNextPage || undefined}
          onScroll={onScroll}
          className={cn(styles.list, history.isPending && styles.loadingList)}
        >
          {history.isPending ? (
            <StateView mode="loading" placement="panel"
              title={t("quickPaste.loading.title")}
            />
          ) : historyError === "offline" ? (
            <StateView
              mode="offline"
              placement="panel"
              icon="plug"
              title={t("quickPaste.offline.title")}
              description={t("quickPaste.offline.body")}
              actions={<Button variant="secondary" icon="play" onClick={() => void restart()}>{t("quickPaste.offline.action")}</Button>}
            />
          ) : historyError === "not_ready" ? (
            <StateView
              mode="loading"
              placement="panel"
              icon="plug"
              title={t("quickPaste.starting.title")}
              description={t("quickPaste.starting.body")}
            />
          ) : history.error ? (
            <StateView
              mode="error"
              placement="panel"
              icon="alert"
              title={t("quickPaste.failed.title")}
              description={t("quickPaste.failed.body")}
              actions={<Button variant="secondary" icon="refresh" onClick={() => void refetch()}>{t("common.tryAgain")}</Button>}
            />
          ) : items.length === 0 ? (
            <StateView
              mode="empty"
              placement="panel"
              icon={searching ? "searchX" : "library"}
              title={
                searching
                  ? t("quickPaste.noResults.title", { query })
                  : t("quickPaste.empty.title")
              }
              description={t(searching ? "quickPaste.noResults.body" : "quickPaste.empty.body")}
            />
          ) : (
            <div className={styles.virtualCanvas} style={{ height: virtualizer.getTotalSize() }}>
              {virtualRows.map((row) => {
                const item = items[row.index];
                if (!item) {
                  return (
                    <div
                      key={row.key}
                      ref={virtualizer.measureElement}
                      data-index={row.index}
                      role="status"
                      className={styles.loadingMore}
                      style={{ transform: `translateY(${row.start}px)` }}
                    >
                      {history.isFetchingNextPage ? <StateView mode="loading" placement="control" role="none" title={t("quickPaste.loadingMore")} /> : (
                        <Button type="button" variant="ghost" size="sm" onClick={loadMore}>
                          {t("quickPaste.loadMore")}
                        </Button>
                      )}
                    </div>
                  );
                }
                return (
                  <div
                    key={row.key}
                    ref={virtualizer.measureElement}
                    data-index={row.index}
                    className={styles.virtualRow}
                    style={{ transform: `translateY(${row.start}px)` }}
                  >
                    <QuickPasteRow
                      item={item}
                      active={selectedId === item.id}
                      shortcut={!searching && row.index < 9
                        ? acceleratorLabel(`CmdOrCtrl+${row.index + 1}`)
                        : null}
                      pinPending={pinPendingId === item.id}
                      copyPending={copyPending}
                      origin={markedOrigin(item, originMarks)}
                      fullContent={selectedId === item.id ? selectedBody.text : null}
                      fullContentFailed={selectedId === item.id && selectedBody.failed}
                      onSelect={() => selectFromPointer(item.id)}
                      onSelectFromKeyboard={() => selectFromKeyboard(item.id)}
                      onCopy={(plainText) => void copyAndDismiss(item, plainText)}
                      onTogglePin={() => void changePin(item.id, !item.pinned)}
                    />
                  </div>
                );
              })}
            </div>
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
                      total: history.total ?? loadedHistory.items.length,
                    })
                  : (history.total ?? 0) > items.length
                    ? t("quickPaste.count.partial", {
                        shown: items.length,
                        total: history.total ?? loadedHistory.items.length,
                      })
                    : t("quickPaste.count.all", { count: items.length })}
            </p>
            {copyPending ? (
              <StateView mode="loading" placement="control" data-state="pending" title={t("quickPaste.copying")} />
            ) : null}
            <Button
              size="compactIcon"
              variant="ghost"
              icon="settings"
              label={t("quickPaste.settings")}
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
    {previewLayout !== null && selectedItem !== null ? (
      <QuickPastePreview
        item={selectedItem}
        fullContent={selectedBody.text}
        fullContentFailed={selectedBody.failed}
        layout={previewLayout}
      />
    ) : null}
    </main>
  );
}
