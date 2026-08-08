import { Search, X } from "lucide-react";
import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useTranslation } from "@/i18n";
import { rankFuzzy } from "@/lib/fuzzy";
import type { SettingsSearchItem, SettingsSearchTab } from "@/components/settings/settingsSearchIndex";

export interface ResolvedSettingsSearchItem {
  item: SettingsSearchItem;
  tabLabel: string;
  sectionLabel?: string;
  title: string;
  description?: string;
}

interface SettingsSearchFieldProps {
  query: string;
  onQueryChange: (value: string) => void;
  results: readonly ResolvedSettingsSearchItem[];
  onSelect: (item: ResolvedSettingsSearchItem) => void;
}

interface SettingsSearchResultsProps {
  results: readonly ResolvedSettingsSearchItem[];
  onSelect: (item: ResolvedSettingsSearchItem) => void;
}

export function SettingsSearchField({
  query,
  onQueryChange,
  results,
  onSelect,
}: SettingsSearchFieldProps) {
  const { t } = useTranslation();
  const hasQuery = query.trim().length > 0;
  const anchorRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const resultsRef = useRef<HTMLDivElement>(null);
  const resultIds = results.map((_, index) => `settings-search-result-${index}`);

  useEffect(() => {
    if (!hasQuery) return;

    const closeOnOutsidePointer = (event: PointerEvent) => {
      const target = event.target as Node;
      if (anchorRef.current?.contains(target) || resultsRef.current?.contains(target)) return;
      onQueryChange("");
    };
    document.addEventListener("pointerdown", closeOnOutsidePointer, true);
    return () => document.removeEventListener("pointerdown", closeOnOutsidePointer, true);
  }, [hasQuery, onQueryChange]);

  const focusResult = (index: number) => {
    document.getElementById(resultIds[index])?.focus();
  };

  return (
    <div
      ref={anchorRef}
      role="search"
      className="relative w-full min-w-0 flex-1 basis-full"
    >
      <Search
        aria-hidden="true"
        className="pointer-events-none absolute inset-y-0 left-s-3 my-auto size-4 text-muted-foreground"
      />
      <Input
        ref={inputRef}
        type="text"
        role="searchbox"
        value={query}
        onChange={(event) => onQueryChange(event.target.value)}
        aria-label={t("settings.search.label")}
        aria-controls="settings-search-results"
        aria-expanded={hasQuery}
        aria-autocomplete="list"
        placeholder={t("settings.search.placeholder")}
        className="pr-11 pl-9"
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            if (hasQuery) {
              event.preventDefault();
              onQueryChange("");
            }
          } else if (event.key === "ArrowDown" && hasQuery && resultIds.length > 0) {
            event.preventDefault();
            focusResult(0);
          } else if (event.key === "ArrowUp" && hasQuery && resultIds.length > 0) {
            event.preventDefault();
            focusResult(resultIds.length - 1);
          }
        }}
      />
      {hasQuery && (
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={t("settings.search.clear")}
          onClick={() => onQueryChange("")}
          className="absolute inset-y-0 right-0 my-auto"
        >
          <X aria-hidden="true" />
        </Button>
      )}
      {hasQuery && (
        <SettingsSearchResults
          anchorRef={anchorRef}
          inputRef={inputRef}
          resultsRef={resultsRef}
          results={results}
          resultIds={resultIds}
          onSelect={onSelect}
        />
      )}
    </div>
  );
}

interface SettingsSearchResultsPosition {
  left: number;
  top: number;
  width: number;
  maxHeight: number;
}

export function SettingsSearchResults({
  anchorRef,
  inputRef,
  resultsRef,
  results,
  resultIds,
  onSelect,
}: SettingsSearchResultsProps & {
  anchorRef: React.RefObject<HTMLDivElement | null>;
  inputRef: React.RefObject<HTMLInputElement | null>;
  resultsRef: React.RefObject<HTMLDivElement | null>;
  resultIds: readonly string[];
}) {
  const { t } = useTranslation();
  const [position, setPosition] = useState<SettingsSearchResultsPosition>({
    left: 8,
    top: 8,
    width: 320,
    maxHeight: 384,
  });
  const resultId = useId();

  useLayoutEffect(() => {
    const updatePosition = () => {
      const anchor = anchorRef.current;
      if (!anchor) return;
      const rect = anchor.getBoundingClientRect();
      const inset = 8;
      const gap = 8;
      const viewportWidth = window.innerWidth;
      const viewportHeight = window.innerHeight;
      const width = Math.min(Math.max(rect.width, 280), viewportWidth - inset * 2);
      const left = Math.min(Math.max(inset, rect.left), viewportWidth - inset - width);
      const below = viewportHeight - rect.bottom - gap;
      const above = rect.top - gap;
      const openAbove = below < 160 && above > below;
      const availableHeight = Math.max(120, Math.min(384, openAbove ? above : below));
      setPosition({
        left,
        top: openAbove ? Math.max(inset, rect.top - gap - availableHeight) : rect.bottom + gap,
        width,
        maxHeight: availableHeight,
      });
    };

    updatePosition();
    const resizeObserver = new ResizeObserver(updatePosition);
    if (anchorRef.current) resizeObserver.observe(anchorRef.current);
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      resizeObserver.disconnect();
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [anchorRef]);

  const select = (result: ResolvedSettingsSearchItem) => onSelect(result);

  const content = (
    <div
      ref={resultsRef}
      id="settings-search-results"
      role="region"
      aria-label={t("settings.search.results")}
      data-settings-search-portal="true"
      style={{
        left: position.left,
        top: position.top,
        width: position.width,
        maxHeight: position.maxHeight,
      }}
      className="fixed z-[var(--z-popover)] overflow-hidden rounded-xl border border-border-strong bg-popover shadow-2 motion-safe:animate-in motion-safe:fade-in-0 motion-safe:zoom-in-95"
    >
      {results.length === 0 ? (
        <div className="flex flex-col gap-0 px-s-3 py-s-2">
          <p className="text-sm leading-5 font-medium">{t("settings.search.empty.title")}</p>
          <p className="text-xs leading-4 text-muted-foreground">
            {t("settings.search.empty.body")}
          </p>
        </div>
      ) : (
        <ul className="max-h-full overflow-y-auto overscroll-contain py-s-1">
          {results.map((result, index) => (
            <li key={`${result.item.tab}:${result.title}`}>
              <button
                id={resultIds[index]}
                type="button"
                onClick={() => select(result)}
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    event.preventDefault();
                    inputRef.current?.focus();
                    return;
                  }
                  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
                  event.preventDefault();
                  const delta = event.key === "ArrowDown" ? 1 : -1;
                  const next = (index + delta + resultIds.length) % resultIds.length;
                  document.getElementById(resultIds[next])?.focus();
                }}
                className="flex w-full flex-col gap-0 px-s-3 py-s-2 text-left hover:bg-selected focus-visible:bg-selected focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
              >
                <span className="text-xs leading-4 text-muted-foreground">
                  {result.tabLabel}
                  {result.sectionLabel && ` · ${result.sectionLabel}`}
                </span>
                <span className="text-sm leading-5 font-medium">{result.title}</span>
                {result.description && (
                  <span className="line-clamp-1 text-xs leading-4 text-muted-foreground">
                    {result.description}
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
  return createPortal(content, document.body, resultId);
}

/** Scored per field rather than over one joined haystack, so "storage quota"
 *  ranks the row it names above one that merely contains both words. */
export function resolveSettingsSearch(
  items: readonly SettingsSearchItem[],
  tabLabels: ReadonlyMap<SettingsSearchTab, string>,
  translate: (key: string) => string,
  query: string,
): readonly ResolvedSettingsSearchItem[] {
  if (query.trim().length === 0) return [];

  const resolved = items.map<ResolvedSettingsSearchItem>((item) => ({
    item,
    tabLabel: tabLabels.get(item.tab) ?? item.tab,
    sectionLabel: item.section ? translate(item.section) : undefined,
    title: translate(item.title),
    description: item.description ? translate(item.description) : undefined,
  }));

  return rankFuzzy(resolved, query, (result) => [
    result.title,
    result.description,
    result.sectionLabel,
    result.tabLabel,
    ...(result.item.keywords ?? []),
  ]);
}
