import * as PopoverPrimitive from "@radix-ui/react-popover";
import { Search, X } from "lucide-react";
import { useRef } from "react";

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

  const focusResult = (index: number) => {
    document.getElementById(resultIds[index])?.focus();
  };

  return (
    <PopoverPrimitive.Root
      open={hasQuery}
      onOpenChange={(open) => {
        if (!open) onQueryChange("");
      }}
    >
      <PopoverPrimitive.Anchor asChild>
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
        </div>
      </PopoverPrimitive.Anchor>
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
    </PopoverPrimitive.Root>
  );
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
  const select = (result: ResolvedSettingsSearchItem) => onSelect(result);

  return (
    <PopoverPrimitive.Portal>
      <PopoverPrimitive.Content
        ref={resultsRef}
        id="settings-search-results"
        role="region"
        aria-label={t("settings.search.results")}
        data-settings-search-portal="true"
        sideOffset={8}
        align="start"
        collisionPadding={8}
        onOpenAutoFocus={(event) => event.preventDefault()}
        onFocusOutside={(event) => event.preventDefault()}
        onPointerDownOutside={(event) => {
          if (anchorRef.current?.contains(event.target as Node)) event.preventDefault();
        }}
        onEscapeKeyDown={(event) => {
          if (resultsRef.current?.contains(event.target as Node)) {
            event.preventDefault();
            inputRef.current?.focus();
          }
        }}
        className="z-[var(--z-popover)] max-h-[min(384px,var(--radix-popover-content-available-height))] w-[max(var(--radix-popover-trigger-width),280px)] max-w-[calc(100vw-16px)] overflow-hidden rounded-xl border border-border-strong bg-popover shadow-2 motion-safe:animate-in motion-safe:fade-in-0 motion-safe:zoom-in-95"
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
      </PopoverPrimitive.Content>
    </PopoverPrimitive.Portal>
  );
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
