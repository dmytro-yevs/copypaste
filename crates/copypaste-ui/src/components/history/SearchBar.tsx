/**
 * `historyCount` is the only place that decides which number the badge shows.
 * v1 had more than one, and the badge read "14 items" beside a search matching
 * none of them (CopyPaste-g27b.37).
 *
 * The shortcut hint lives in `title`: a permanently visible one crowded the
 * header and read as disabled text (CopyPaste-7w060.6).
 *
 * # Why the filter and sort controls are native `<select>`s
 *
 * They are the one control the platform already renders as a picker on a phone
 * and as a menu on a desktop, with keyboard behaviour, type-ahead and screen
 * reader support that no popup we assemble would match. A Radix dropdown would
 * be a bigger dependency and a worse Android experience. The class list only
 * restyles the closed state — the open list stays native on purpose.
 */
import type { RefObject } from "react";
import { ListChecks, Search, Trash2, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/cn";
import {
  FILTERABLE_KINDS,
  KIND_LABEL,
  SORT_LABEL,
  type KindFilter,
  type SortOrder,
  type ViewOptions,
} from "@/lib/view";

/** The filtered count whenever a filter is active, the service's total
 *  otherwise (AT-68). */
export function historyCount(
  filtered: boolean,
  visible: number,
  total: number | undefined,
): string {
  const n = filtered || total === undefined ? visible : total;
  return `${n} item${n === 1 ? "" : "s"}`;
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
          placeholder="Search clipboard history"
          aria-label="Search clipboard history"
          title="Search (⌘F) · ↓ to move into the list · ⌘A select all"
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
            aria-label="Clear search"
            title="Clear search"
            className="absolute right-0.5"
            onClick={() => onChange("")}
          >
            <X aria-hidden="true" />
          </Button>
        )}
      </div>

      <select
        aria-label="Filter by kind"
        className={SELECT_CLASS}
        value={view.kind}
        onChange={(event) =>
          onViewChange({ ...view, kind: event.target.value as KindFilter })
        }
      >
        <option value="all">{KIND_LABEL.all}</option>
        {FILTERABLE_KINDS.map((kind) => (
          <option key={kind} value={kind}>
            {KIND_LABEL[kind]}
          </option>
        ))}
      </select>

      <select
        aria-label="Sort order"
        className={SELECT_CLASS}
        value={view.sort}
        onChange={(event) =>
          onViewChange({ ...view, sort: event.target.value as SortOrder })
        }
      >
        <option value="newest">{SORT_LABEL.newest}</option>
        <option value="oldest">{SORT_LABEL.oldest}</option>
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
        aria-label={selecting ? "Leave selection mode" : "Select multiple items"}
        aria-pressed={selecting}
        title={selecting ? "Leave selection mode" : "Select multiple items"}
        className={cn(selecting && "bg-selected text-foreground")}
        onClick={onToggleSelecting}
      >
        <ListChecks aria-hidden="true" />
      </Button>

      {onClearAll && (
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Clear clipboard history"
          title="Clear clipboard history"
          onClick={onClearAll}
        >
          <Trash2 aria-hidden="true" />
        </Button>
      )}
    </div>
  );
}
