import * as PopoverPrimitive from "@radix-ui/react-popover";
import {
  useEffect,
  useId,
  useRef,
  useState,
  type RefObject,
} from "react";

import { SearchField } from "@/components/shared";
import { Button, Surface, VisuallyHidden } from "@/components/ui";
import { ExpandableSearch } from "@/features/settings/components/ExpandableSearch";
import type {
  SettingsSearchItem,
  SettingsSearchTab,
} from "@/features/settings/model/settingsSearchIndex";
import { useTranslation } from "@/i18n";
import { rankFuzzy } from "@/lib/fuzzy";
import styles from "./SettingsSearch.module.css";

export interface ResolvedSettingsSearchItem {
  item: SettingsSearchItem;
  groupLabel?: string;
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
  inputRef: RefObject<HTMLInputElement | null>;
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
}

export function SettingsSearchField({
  query,
  onQueryChange,
  results,
  onSelect,
  inputRef,
  expanded,
  onExpandedChange,
}: SettingsSearchFieldProps) {
  const { t } = useTranslation();
  const id = useId().replace(/:/g, "");
  const listId = `settings-search-results-${id}`;
  const resultIds = results.map((_, index) => `${listId}-option-${index}`);
  const [popoverOpen, setPopoverOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);
  const [countAnnouncement, setCountAnnouncement] = useState("");
  const triggerRef = useRef<HTMLButtonElement>(null);
  const hasQuery = query.trim().length > 0;

  const collapseAndRestore = () => {
    setPopoverOpen(false);
    onExpandedChange(false);
    requestAnimationFrame(() => triggerRef.current?.focus());
  };

  useEffect(() => {
    setActiveIndex(-1);
    if (hasQuery) setPopoverOpen(true);
  }, [hasQuery, query]);

  useEffect(() => {
    if (!hasQuery) {
      setCountAnnouncement("");
      return;
    }
    const timer = window.setTimeout(() => {
      setCountAnnouncement(
        results.length === 1
          ? "1 setting found"
          : `${results.length} settings found`,
      );
    }, 350);
    return () => window.clearTimeout(timer);
  }, [hasQuery, results.length]);

  const select = (result: ResolvedSettingsSearchItem) => {
    setPopoverOpen(false);
    onSelect(result);
  };

  return (
    <PopoverPrimitive.Root
      open={popoverOpen && hasQuery}
      onOpenChange={setPopoverOpen}
    >
      <PopoverPrimitive.Anchor asChild>
        <ExpandableSearch
          expanded={expanded}
          label={t("settings.search.label")}
          triggerRef={triggerRef}
          onExpandedChange={(next) => {
            onExpandedChange(next);
            if (next) {
              if (hasQuery) setPopoverOpen(true);
              requestAnimationFrame(() => inputRef.current?.focus());
            }
          }}
          className={styles.search}
        >
          <SearchField
            inputRef={inputRef}
            mode="overlay"
            size="compact"
            expanded
            shortcut=""
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            onClear={() => onQueryChange("")}
            onRequestClose={collapseAndRestore}
            clearLabel={t("settings.search.clear")}
            closeLabel={t("common.close")}
            aria-label={t("settings.search.label")}
            aria-controls={hasQuery ? listId : undefined}
            aria-expanded={popoverOpen && hasQuery}
            aria-autocomplete="list"
            aria-activedescendant={
              popoverOpen && hasQuery ? resultIds[activeIndex] : undefined
            }
            placeholder={t("settings.search.placeholder")}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                if (popoverOpen) {
                  setPopoverOpen(false);
                } else if (hasQuery) {
                  onQueryChange("");
                } else {
                  collapseAndRestore();
                }
                return;
              }
              if (event.key === "ArrowDown" && results.length > 0) {
                event.preventDefault();
                setPopoverOpen(true);
                setActiveIndex((index) => (index + 1) % results.length);
                return;
              }
              if (event.key === "ArrowUp" && results.length > 0) {
                event.preventDefault();
                setPopoverOpen(true);
                setActiveIndex((index) =>
                  index < 0
                    ? results.length - 1
                    : (index - 1 + results.length) % results.length,
                );
                return;
              }
              if (event.key === "Enter" && popoverOpen && results[activeIndex]) {
                event.preventDefault();
                select(results[activeIndex]);
              }
            }}
          />
        </ExpandableSearch>
      </PopoverPrimitive.Anchor>

      {hasQuery ? (
        <SettingsSearchResults
          listId={listId}
          inputRef={inputRef}
          results={results}
          resultIds={resultIds}
          activeIndex={activeIndex}
          onActiveIndexChange={setActiveIndex}
          onClose={() => setPopoverOpen(false)}
          onSelect={select}
        />
      ) : null}

      <VisuallyHidden role="status" aria-live="polite">
        {countAnnouncement}
      </VisuallyHidden>
    </PopoverPrimitive.Root>
  );
}

export function SettingsSearchResults({
  listId,
  inputRef,
  results,
  resultIds,
  activeIndex,
  onActiveIndexChange,
  onClose,
  onSelect,
}: {
  listId: string;
  inputRef: RefObject<HTMLInputElement | null>;
  results: readonly ResolvedSettingsSearchItem[];
  resultIds: readonly string[];
  activeIndex: number;
  onActiveIndexChange: (index: number) => void;
  onClose: () => void;
  onSelect: (item: ResolvedSettingsSearchItem) => void;
}) {
  const { t } = useTranslation();
  const contentRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (activeIndex < 0) return;
    document.getElementById(resultIds[activeIndex] ?? "")?.scrollIntoView({
      block: "nearest",
    });
  }, [activeIndex, resultIds]);

  return (
    <PopoverPrimitive.Portal>
      <Surface asChild elevation="overlay" border="subtle" radius="md">
        <PopoverPrimitive.Content
          ref={contentRef}
          sideOffset={8}
          align="end"
          collisionPadding={8}
          onOpenAutoFocus={(event) => event.preventDefault()}
          onEscapeKeyDown={(event) => {
            event.preventDefault();
            onClose();
            inputRef.current?.focus();
          }}
          className={styles.results}
        >
          {results.length === 0 ? (
            <div
              id={listId}
              className={styles.empty}
            >
              <p className={styles.emptyTitle}>No settings found</p>
              <p className={styles.emptyBody}>{t("settings.search.empty.body")}</p>
            </div>
          ) : (
            <ul id={listId} role="listbox" aria-label={t("settings.search.results")} className={styles.resultList}>
              {results.map((result, index) => (
                <li key={`${result.item.tab}:${result.title}`} role="presentation">
                  <Button
                    id={resultIds[index]}
                    type="button"
                    role="option"
                    aria-selected={index === activeIndex}
                    variant="ghost"
                    size="md"
                    onFocus={() => onActiveIndexChange(index)}
                    onPointerMove={() => onActiveIndexChange(index)}
                    onClick={() => onSelect(result)}
                    onKeyDown={(event) => {
                      if (event.key === "Escape") {
                        event.preventDefault();
                        onClose();
                        inputRef.current?.focus();
                        return;
                      }
                      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
                      event.preventDefault();
                      const delta = event.key === "ArrowDown" ? 1 : -1;
                      const next = (index + delta + resultIds.length) % resultIds.length;
                      document.getElementById(resultIds[next])?.focus();
                    }}
                    className={styles.result}
                  >
                    <span className={styles.breadcrumb}>
                      {result.groupLabel ? `${result.groupLabel} · ` : ""}
                      {result.tabLabel}
                      {result.sectionLabel ? ` · ${result.sectionLabel}` : ""}
                    </span>
                    <span className={styles.resultTitle}>{result.title}</span>
                    {result.description ? (
                      <span className={styles.resultDescription}>{result.description}</span>
                    ) : null}
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </PopoverPrimitive.Content>
      </Surface>
    </PopoverPrimitive.Portal>
  );
}

export function resolveSettingsSearch(
  items: readonly SettingsSearchItem[],
  tabLabels: ReadonlyMap<SettingsSearchTab, string>,
  translate: (key: string) => string,
  query: string,
  groupLabels: ReadonlyMap<SettingsSearchTab, string> = new Map(),
): readonly ResolvedSettingsSearchItem[] {
  if (query.trim().length === 0) return [];

  const resolved = items.map<ResolvedSettingsSearchItem>((item) => ({
    item,
    groupLabel: groupLabels.get(item.tab),
    tabLabel: tabLabels.get(item.tab) ?? item.tab,
    sectionLabel: item.section ? translate(item.section) : undefined,
    title: translate(item.title),
    description: item.description ? translate(item.description) : undefined,
  }));

  return rankFuzzy(resolved, query, (result) => [
    result.title,
    result.description,
    result.groupLabel,
    result.sectionLabel,
    result.tabLabel,
    ...(result.item.keywords ?? []),
  ]);
}
