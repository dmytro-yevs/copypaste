import { type RefObject } from "react";
import { Search, X } from "lucide-react";

import { IconButton } from "./IconButton";

interface SearchBarProps {
  value: string;
  onChange: (value: string) => void;
  /** ArrowDown from the field hands control to the list. */
  onEnterList: () => void;
  inputRef: RefObject<HTMLInputElement | null>;
}

/**
 * The result count deliberately lives in the status line and nowhere else.
 * Manifest §3.1.7 (CopyPaste-g27b.37) is about a count that kept reading the
 * daemon total while a search showed zero matches; the way to not reintroduce
 * that is to have exactly one place that decides which number is being shown.
 */
export function SearchBar({
  value,
  onChange,
  onEnterList,
  inputRef,
}: SearchBarProps) {
  return (
    <div className="flex items-center gap-s-3 border-b border-divider bg-panel px-s-4 py-s-3">
      <div className="relative flex min-w-0 flex-1 items-center">
        <Search
          size={14}
          aria-hidden="true"
          className="pointer-events-none absolute left-s-4 text-mute"
        />
        <input
          ref={inputRef}
          type="search"
          value={value}
          spellCheck={false}
          autoComplete="off"
          placeholder="Search clipboard history"
          aria-label="Search clipboard history"
          title="Search (⌘F)"
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
          className="h-[var(--ctl-h-lg)] w-full min-w-0 rounded-input border border-border bg-elevated pr-[var(--ctl-h-lg)] pl-[30px] text-fs-md text-text placeholder:text-faint [&::-webkit-search-cancel-button]:hidden"
        />
        {value && (
          <IconButton
            label="Clear search"
            size="sm"
            className="absolute right-[2px]"
            onClick={() => onChange("")}
          >
            <X size={14} aria-hidden="true" />
          </IconButton>
        )}
      </div>
    </div>
  );
}
