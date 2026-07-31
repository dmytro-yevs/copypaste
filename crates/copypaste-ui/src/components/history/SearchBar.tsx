/**
 * `historyCount` is the only place that decides which number the badge shows.
 * v1 had more than one, and the badge read "14 items" beside a search matching
 * none of them (CopyPaste-g27b.37).
 *
 * The shortcut hint lives in `title`: a permanently visible one crowded the
 * header and read as disabled text (CopyPaste-7w060.6).
 *
 * The filter and sort controls are native `<select>`s: the platform renders one
 * as a picker on a phone and a menu on a desktop, with keyboard, type-ahead and
 * screen reader behaviour a popup we assembled would not match.
 */
import type { RefObject } from "react";
import { ListChecks, Search, Trash2, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { t as translate, useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
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

const SELECT_CLASS =
  "h-8 rounded-md border border-border-strong bg-panel px-2 text-xs text-foreground outline-none focus-visible:ring-[3px] focus-visible:ring-ring";

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
  selecting,
  onToggleSelecting,
  onClearAll,
}: SearchBarProps) {
  const { t } = useTranslation();

  return (
    <div className="flex shrink-0 flex-wrap items-center gap-s-2 border-b border-divider bg-panel px-s-3 py-s-2">
      <div className="relative flex min-w-[180px] flex-1 items-center">
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
            } else if (event.key === "Escape" && value) {
              event.preventDefault();
              onChange("");
            }
          }}
          className="pr-9 pl-8 [&::-webkit-search-cancel-button]:hidden"
        />
        {value && (
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={t("history.search.clear")}
            title={t("history.search.clear")}
            className="absolute right-0.5"
            onClick={() => onChange("")}
          >
            <X aria-hidden="true" />
          </Button>
        )}
      </div>

      <select
        aria-label={t("history.search.filterKind")}
        className={SELECT_CLASS}
        value={view.kind}
        onChange={(event) =>
          onViewChange({ ...view, kind: event.target.value as KindFilter })
        }
      >
        <option value="all">{kindLabel("all")}</option>
        {FILTERABLE_KINDS.map((kind) => (
          <option key={kind} value={kind}>
            {kindLabel(kind)}
          </option>
        ))}
      </select>

      <select
        aria-label={t("history.search.sortOrder")}
        className={SELECT_CLASS}
        value={view.sort}
        onChange={(event) =>
          onViewChange({ ...view, sort: event.target.value as SortOrder })
        }
      >
        <option value="newest">{sortLabel("newest")}</option>
        <option value="oldest">{sortLabel("oldest")}</option>
      </select>

      <span
        className="shrink-0 text-xs tabular-nums text-muted-foreground"
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
    </div>
  );
}
