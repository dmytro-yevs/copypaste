/**
 * `historyCount` is the only place that decides which number the badge shows.
 * v1 had more than one, and the badge read "14 items" beside a search matching
 * none of them (CopyPaste-g27b.37).
 *
 * The shortcut hint lives in `title`: a permanently visible one crowded the
 * header and read as disabled text (CopyPaste-7w060.6).
 *
 * The filter row stays one line wide on Android, so the shared selectors need
 * icon-only triggers there without forking the toolbar or the control.
 */
import type { RefObject } from "react";
import {
  ArrowDown,
  ArrowUp,
  ListChecks,
  MonitorSmartphone,
  Search,
  Trash2,
  X,
} from "lucide-react";

import { clipTypeMetadata } from "@/components/history/clipMetadata";
import { type OriginDevice, originName } from "@/components/history/origin";
import { Button } from "@/components/ui/button";
import { ExpandableSearch } from "@/components/ui/expandable-search";
import { Input } from "@/components/ui/input";
import { Select, type SelectItem } from "@/components/ui/select";
import { useHeaderSearch } from "@/components/ui/use-header-search";
import { t as translate, useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import { isAndroidPlatform } from "@/lib/platform";
import {
  FILTERABLE_KINDS,
  type KindFilter,
  type SortOrder,
  type ViewOptions,
  kindLabel,
  sortLabel,
} from "@/lib/view";

/** The filtered count whenever a filter is active, the service's total
 *  otherwise (AT-68). */
export function historyCount(
  filtered: boolean,
  visible: number,
  total: number | undefined,
): string {
  const n = filtered || total === undefined ? visible : total;
  return translate("history.search.count", { count: n });
}

interface SearchBarProps {
  value: string;
  onChange: (value: string) => void;
  onEnterList: () => void;
  inputRef: RefObject<HTMLInputElement | null>;
  filtered: boolean;
  visible: number;
  total: number | undefined;
  view: ViewOptions;
  onViewChange: (view: ViewOptions) => void;
  origins: readonly OriginDevice[];
  displayLimit: number | null;
  selecting: boolean;
  onToggleSelecting: () => void;
  onClearAll?: () => void;
}

export function SearchBar({
  value,
  onChange,
  onEnterList,
  inputRef,
  filtered,
  visible,
  total,
  view,
  onViewChange,
  origins,
  displayLimit,
  selecting,
  onToggleSelecting,
  onClearAll,
}: SearchBarProps) {
  const { t } = useTranslation();
  const compact = isAndroidPlatform();
  const search = useHeaderSearch(inputRef, compact);
  const kindItems: readonly SelectItem[] = [
    { value: "all", label: kindLabel("all"), icon: Search },
    ...FILTERABLE_KINDS.map((kind) => ({
      value: kind,
      label: kindLabel(kind),
      icon: clipTypeMetadata(kind).Icon,
    })),
  ];
  const deviceItems: readonly SelectItem[] = [
    {
      value: "all",
      label: t("history.search.allDevices"),
      icon: MonitorSmartphone,
    },
    ...origins.map((origin) => ({
      value: origin.id,
      label: originName(origin),
      icon: MonitorSmartphone,
    })),
  ];
  const sortItems: readonly SelectItem[] = [
    { value: "newest", label: sortLabel("newest"), icon: ArrowDown },
    { value: "oldest", label: sortLabel("oldest"), icon: ArrowUp },
  ];

  const searchField = (
    <div className="relative flex min-w-0 flex-1 items-center">
      <Search
        size={14}
        aria-hidden="true"
        className="pointer-events-none absolute left-s-2 text-muted-foreground"
      />
      <Input
        ref={inputRef}
        type="search"
        value={value}
        spellCheck={false}
        autoComplete="off"
        placeholder={t("history.search.placeholder")}
        aria-label={t("history.search.label")}
        title={t("history.search.hint")}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown") {
            event.preventDefault();
            onEnterList();
          } else if (event.key === "Escape") {
            event.preventDefault();
            if (value) onChange("");
            else if (compact) search.setExpanded(false);
          }
        }}
        className="pr-9 pl-8 [&::-webkit-search-cancel-button]:hidden"
      />
      {(value || compact) && (
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={t(
            value ? "history.search.clear" : "history.search.close",
          )}
          title={t(value ? "history.search.clear" : "history.search.close")}
          className="absolute right-0.5"
          onClick={() => {
            if (value) onChange("");
            else if (compact) search.setExpanded(false);
          }}
        >
          <X aria-hidden="true" />
        </Button>
      )}
    </div>
  );

  return (
    <>
      <div
        data-slot="history-toolbar"
        data-search-open={search.expanded}
        className="flex shrink-0 flex-nowrap items-center gap-s-2 border-b border-divider bg-transparent px-s-3 py-s-2"
      >
        <ExpandableSearch
          expanded={search.expanded}
          label={t("history.search.open")}
          onExpandedChange={(expanded) => {
            if (expanded) search.open();
            else search.setExpanded(false);
          }}
          className={compact ? undefined : "w-auto basis-auto"}
        >
          {searchField}
        </ExpandableSearch>

        {(!compact || !search.expanded) && (
          <>
            <Select
              aria-label={t("history.search.filterKind")}
              className="shrink-0"
              value={view.kind}
              items={kindItems}
              onValueChange={(kind) =>
                onViewChange({
                  ...view,
                  kind: kind as KindFilter,
                })
              }
            />

            {origins.length > 1 && (
              <Select
                aria-label={t("history.search.filterDevice")}
                className="shrink-0"
                value={view.device}
                items={deviceItems}
                onValueChange={(device) =>
                  onViewChange({
                    ...view,
                    device,
                  })
                }
              />
            )}

            <Select
              aria-label={t("history.search.sortOrder")}
              className="shrink-0"
              value={view.sort}
              items={sortItems}
              onValueChange={(sort) =>
                onViewChange({
                  ...view,
                  sort: sort as SortOrder,
                })
              }
            />

            {origins.length > 1 && (
              <Button
                variant="ghost"
                size="sm"
                aria-label={t("history.search.groupByDevice")}
                aria-pressed={view.groupByDevice}
                title={t("history.search.groupByDevice")}
                className={cn(
                  "h-8",
                  view.groupByDevice && "bg-selected text-foreground",
                )}
                onClick={() =>
                  onViewChange({
                    ...view,
                    groupByDevice: !view.groupByDevice,
                  })
                }
              >
                <MonitorSmartphone aria-hidden="true" />
              </Button>
            )}

            <span
              className="sr-only shrink-0 text-xs tabular-nums text-muted-foreground"
              aria-live="polite"
            >
              {historyCount(filtered, visible, total)}
            </span>

            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={t(
                selecting
                  ? "history.search.leaveSelection"
                  : "history.search.selectMultiple",
              )}
              aria-pressed={selecting}
              title={t(
                selecting
                  ? "history.search.leaveSelection"
                  : "history.search.selectMultiple",
              )}
              className={cn(selecting && "bg-selected text-foreground")}
              onClick={onToggleSelecting}
            >
              <ListChecks aria-hidden="true" />
            </Button>

            {onClearAll && (
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={t("history.search.clearAll")}
                title={t("history.search.clearAll")}
                onClick={onClearAll}
              >
                <Trash2 aria-hidden="true" />
              </Button>
            )}
          </>
        )}
      </div>

      {displayLimit !== null && (
        <p
          aria-live="polite"
          className="shrink-0 border-b border-divider px-s-3 py-s-1 text-xs text-muted-foreground"
        >
          {t("history.search.displayLimitHint", {
            limit: displayLimit,
            count: visible,
          })}
        </p>
      )}
    </>
  );
}
