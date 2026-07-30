/**
 * `historyCount` is the only place that decides which number the badge shows.
 * v1 had more than one, and the badge read "14 items" beside a search matching
 * none of them (CopyPaste-g27b.37).
 *
 * The shortcut hint lives in `title`: a permanently visible one crowded the
 * header and read as disabled text (CopyPaste-7w060.6).
 */
import type { RefObject } from "react";
import { Search, Trash2, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

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

interface SearchBarProps {
  value: string;
  onChange: (value: string) => void;
  onEnterList: () => void;
  inputRef: RefObject<HTMLInputElement | null>;
  filtered: boolean;
  visible: number;
  total: number | undefined;
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
          title="Search (⌘F) · ↓ to move into the list"
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

      <span
        className="shrink-0 text-xs tabular-nums text-muted-foreground"
        aria-live="polite"
      >
        {historyCount(filtered, visible, total)}
      </span>

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
