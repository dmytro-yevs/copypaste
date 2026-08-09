import { useCallback, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  ClipboardList,
  Play,
  PlugZap,
  RefreshCw,
  Search,
  SearchX,
  Settings2,
  TriangleAlert,
} from "lucide-react";
import { toast } from "sonner";

import { EmptyState } from "@/components/EmptyState";
import { QuickPasteRow } from "@/components/quick-paste/QuickPasteRow";
import {
  QUICK_PASTE_QUERY_KEY,
  useQuickPasteLifecycle,
} from "@/components/quick-paste/useQuickPasteLifecycle";
import { useQuickPasteSelection } from "@/components/quick-paste/useQuickPasteSelection";
import { Input } from "@/components/ui/input";
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
import { rankFuzzy } from "@/lib/fuzzy";
import { POLL_ACTIVE_MS, POLL_BACKOFF_MS } from "@/lib/layout";
import { isAndroid } from "@/lib/platform";

const LIMIT = 50;

/** What the fuzzy search matches against. A flagged clip is matched as its mask
 *  and not its text, so a search can never confirm a withheld value. */
function searchLabel(item: Item): string {
  if (item.is_sensitive) return "••••••••";
  if (item.content_type.toLowerCase().startsWith("image/")) return t("quickPaste.row.image");
  if (item.content_type.toLowerCase() === "file") return t("quickPaste.row.file");
  if (item.sensitive_finding) return item.sensitive_finding.redacted_preview;
  return item.content ?? "";
}

/**
 * A deliberately compact, standalone surface. It has its own data lifecycle:
 * results are discarded when the popup loses focus and re-fetched when it is
 * shown, so a warm WebView cannot show a stale clipboard list.
 */
export function QuickPasteApp() {
  const android = isAndroid();
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
    historyPreviewLines,
    dismiss,
    dismissOnRootBlur,
    currentCacheGeneration,
    isCacheGenerationCurrent,
  } = useQuickPasteLifecycle({ searchRef, clearLocalState });

  const history = useQuery({
    queryKey: QUICK_PASTE_QUERY_KEY,
    queryFn: () => listItems(LIMIT, null),
    enabled: holding,
    refetchInterval: (q) =>
      q.state.status === "error" ? POLL_BACKOFF_MS : POLL_ACTIVE_MS,
    // Showing the popup is driven explicitly below, and React Query's own focus
    // refetch would read the daemon a second time for the same appearance.
    refetchOnWindowFocus: false,
  });
  const { refetch } = history;

  const items = useMemo(
    () => rankFuzzy(history.data?.items ?? [], query, (item) => [searchLabel(item)]),
    [history.data?.items, query],
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
        // Drop the popup cache before it is hidden. The next invocation always
        // starts from a daemon read, and the Accessory hide path restores the
        // app the user was about to paste into.
        dismiss();
      } catch (error: unknown) {
        console.error("Quick Paste copy failed", error);
        if (!isCacheGenerationCurrent(generation)) return;
        // Keep the result visible rather than pretending the clipboard changed.
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

  const historyError = history.error ? classifyError(history.error) : null;
  const searching = query.trim().length > 0;

  return (
    <main
      aria-label={t("quickPaste.title")}
      className="flex h-full min-h-0 flex-col overflow-hidden rounded-2xl border border-border bg-background p-3 text-foreground shadow-xl"
      onKeyDown={onKeyDown}
      onBlur={dismissOnRootBlur}
    >
      <div className="relative mb-2 flex items-center gap-2">
        <Search
          size={17}
          aria-hidden="true"
          className="pointer-events-none absolute left-3 text-muted-foreground"
        />
        <Input
          ref={searchRef}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          aria-label={t("quickPaste.search.label")}
          placeholder={t("quickPaste.search.label")}
          className="h-11 flex-1 rounded-xl bg-secondary py-0 pl-9 pr-3"
        />
      </div>

      <div ref={listRef} role="list" onScroll={noteScroll} className="min-h-0 flex-1 overflow-y-auto overscroll-contain pr-0.5">
        {history.isPending ? (
          <EmptyState
            busy
            title={t("quickPaste.loading.title")}
            body={t("quickPaste.loading.body")}
          />
        ) : historyError === "offline" ? (
          <EmptyState
            icon={PlugZap}
            tone="attention"
            title={t("quickPaste.offline.title")}
            body={t("quickPaste.offline.body")}
            action={{
              label: t("quickPaste.offline.action"),
              icon: Play,
              onClick: () => void restart(),
            }}
          />
        ) : historyError === "not_ready" ? (
          <EmptyState
            busy
            icon={PlugZap}
            tone="info"
            title={t("quickPaste.starting.title")}
            body={t("quickPaste.starting.body")}
          />
        ) : history.error ? (
          <EmptyState
            icon={TriangleAlert}
            tone="danger"
            title={t("quickPaste.failed.title")}
            body={t("quickPaste.failed.body")}
            action={{
              label: t("common.tryAgain"),
              icon: RefreshCw,
              onClick: () => void refetch(),
            }}
          />
        ) : items.length === 0 ? (
          <EmptyState
            icon={searching ? SearchX : ClipboardList}
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
              rowPreviewLines={historyPreviewLines}
              shortcut={!android && !searching && index < 9 ? `⌘${index + 1}` : null}
              pinPending={pinPendingId === item.id}
              onSelect={() => selectFromPointer(item.id)}
              onCopy={() => void copyAndDismiss(item)}
              onTogglePin={() => void changePin(item.id, !item.pinned)}
            />
          ))
        )}
      </div>

      <footer className="mt-2 flex h-8 shrink-0 items-center gap-2 border-t border-border pt-1 text-xs text-muted-foreground">
        <button
          type="button"
          aria-label={t("quickPaste.settings")}
          title={t("quickPaste.settings")}
          className="flex size-[var(--sz-iconbtn)] shrink-0 items-center justify-center rounded-md hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
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
        >
          <Settings2 size={16} aria-hidden="true" />
        </button>
        <p aria-live="polite" className="shrink-0 tabular-nums">
          {history.isPending
            ? t("quickPaste.count.loading")
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
      </footer>
    </main>
  );
}
