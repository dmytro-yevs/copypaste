import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  PaneHeader,
  PaneHeaderActions,
  PaneHeaderBackButton,
  PaneHeaderTitle,
} from "@/components/ui/pane-header";
import { useHeaderSearch } from "@/components/ui/use-header-search";
import { useTranslation } from "@/i18n";
import { SettingsIndex } from "@/components/settings/SettingsIndex";
import { useSettingsLevel } from "@/components/settings/useSettingsLevel";
import {
  groupedTabs,
  visibleTabs,
  type SettingsTab,
} from "@/components/settings/settingsTabs";
import {
  resolveSettingsSearch,
  SettingsSearchField,
  type ResolvedSettingsSearchItem,
} from "@/components/settings/SettingsSearch";
import {
  SETTINGS_SEARCH_ITEMS,
  type SettingsSearchTab,
} from "@/components/settings/settingsSearchIndex";
import { useSizeClass } from "@/hooks/useSizeClass";
import { cn } from "@/lib/cn";
import { isAndroid, isWindows } from "@/lib/platform";
import { useUi } from "@/store/ui";

function TabButton({ tab }: { tab: SettingsTab }) {
  const { t } = useTranslation();
  const Icon = tab.icon;

  return (
    <TabsTrigger
      value={tab.value}
      className="w-full justify-start rounded-lg px-s-2 text-left data-[state=active]:bg-muted data-[state=active]:shadow-none"
    >
      <Icon aria-hidden="true" />
      {t(tab.label)}
    </TabsTrigger>
  );
}

export function SettingsView() {
  const { t } = useTranslation();
  const android = isAndroid();
  const compact = useSizeClass() === "compact";
  const visiblePlatform = android ? "android" : isWindows() ? "windows" : "desktop";
  const [activeTab, setActiveTab] = useState<SettingsSearchTab>("appearance");
  const [subpage, setSubpage] = useState<SettingsSearchTab | null>(null);
  const requestedTab = useUi((state) => state.settingsTab);
  const setSettingsTab = useUi((state) => state.setSettingsTab);
  const [query, setQuery] = useState("");
  const searchInputRef = useRef<HTMLInputElement>(null);
  const headerSearch = useHeaderSearch(searchInputRef, true);
  const [searchAnnouncement, setSearchAnnouncement] = useState("");
  const highlightTimer = useRef<number | undefined>(undefined);
  const deferredQuery = useDeferredValue(query);
  const tabs = useMemo(() => visibleTabs(android), [android]);
  const groups = useMemo(() => groupedTabs(tabs), [tabs]);
  const closeSubpage = useCallback(() => setSubpage(null), []);
  const goBack = useSettingsLevel(compact && subpage !== null, closeSubpage);
  const openTab = useCallback((value: SettingsSearchTab) => {
    setActiveTab(value);
    setSubpage(value);
  }, []);
  const tabLabels = useMemo(
    () => new Map(tabs.map((tab) => [tab.value, t(tab.label)])),
    [t, tabs],
  );
  const results = useMemo(
    () =>
      resolveSettingsSearch(
        SETTINGS_SEARCH_ITEMS.filter(
          (item) =>
            tabLabels.has(item.tab) &&
            (!item.platforms ||
              item.platforms.includes(visiblePlatform) ||
              (!android && item.platforms.includes("desktop"))),
        ),
        tabLabels,
        (key) => t(key as never),
        deferredQuery,
      ),
    [android, deferredQuery, t, tabLabels, visiblePlatform],
  );
  useEffect(() => {
    if (requestedTab === null) return;
    if (tabs.some((tab) => tab.value === requestedTab)) {
      openTab(requestedTab as SettingsSearchTab);
    }
    setSettingsTab(null);
  }, [openTab, requestedTab, setSettingsTab, tabs]);

  useEffect(
    () => () => {
      if (highlightTimer.current !== undefined) {
        window.clearTimeout(highlightTimer.current);
      }
    },
    [],
  );

  const selectResult = (result: ResolvedSettingsSearchItem) => {
    openTab(result.item.tab);
    setQuery("");
    headerSearch.setExpanded(false);
    setSearchAnnouncement(t("settings.search.opened", { title: result.title }));
    window.requestAnimationFrame(() => {
      window.requestAnimationFrame(() => {
        const targetIds = [
          `row:${result.title}`,
          `section:${result.title}`,
          result.sectionLabel && `section:${result.sectionLabel}`,
        ];
        const target = [...document.querySelectorAll<HTMLElement>("[data-settings-search-target]")]
          .find((element) => targetIds.includes(element.dataset.settingsSearchTarget));
        if (!target) return;

        target.scrollIntoView({ behavior: "smooth", block: "center" });
        target.tabIndex = -1;
        target.focus({ preventScroll: true });
        target.dataset.settingsSearchHighlight = "true";
        if (highlightTimer.current !== undefined) {
          window.clearTimeout(highlightTimer.current);
        }
        highlightTimer.current = window.setTimeout(() => {
          delete target.dataset.settingsSearchHighlight;
          highlightTimer.current = undefined;
        }, 1800);
      });
    });
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PaneHeader
        aria-labelledby="settings-page-title"
        data-search-open={headerSearch.expanded ? "true" : "false"}
      >
        {!headerSearch.expanded && compact && subpage !== null ? (
          <PaneHeaderBackButton label={t("settings.index.back")} onClick={goBack} />
        ) : null}
        <PaneHeaderTitle id="settings-page-title" className={cn(headerSearch.expanded && "sr-only")}>
          {compact && subpage !== null
            ? t(tabs.find((tab) => tab.value === subpage)?.label ?? "settings.title")
            : t("settings.title")}
        </PaneHeaderTitle>
        <PaneHeaderActions className={cn("ml-auto", headerSearch.expanded && "w-full flex-1")}>
          <SettingsSearchField
            query={query}
            onQueryChange={setQuery}
            results={results}
            onSelect={selectResult}
            inputRef={searchInputRef}
            expanded={headerSearch.expanded}
            onExpandedChange={headerSearch.setExpanded}
          />
        </PaneHeaderActions>
      </PaneHeader>

      <div className={cn("min-h-0 flex-1 overflow-y-auto", compact && "p-s-3")}>
        {compact ? (
          <SettingsIndex
            groups={groups}
            open={tabs.find((tab) => tab.value === subpage) ?? null}
            onOpen={(value) => openTab(value as SettingsSearchTab)}
          />
        ) : (
          <Tabs
            value={activeTab}
            onValueChange={(value) => setActiveTab(value as SettingsSearchTab)}
            orientation="vertical"
            className="min-h-full flex-row gap-0"
          >
            <TabsList
              aria-label={t("settings.sections")}
              variant="bare"
              className="min-h-full w-60 shrink-0 self-stretch flex-col items-stretch gap-s-4 border-r border-divider px-s-3 py-s-4 text-foreground"
            >
              {groups.map((group) => (
                <div key={group.label} className="flex flex-col gap-1">
                  <p className="pt-s-2 text-[11px] font-medium tracking-wide text-muted-foreground">
                    {t(group.label)}
                  </p>
                  {group.tabs.map((tab) => (
                    <TabButton key={tab.value} tab={tab} />
                  ))}
                </div>
              ))}
            </TabsList>

            <div className="min-h-full min-w-0 flex-1 px-s-4 py-s-3 sm:px-s-5">
              {tabs.map((tab) => (
                <TabsContent key={tab.value} value={tab.value} className="min-h-full">
                  {tab.render()}
                </TabsContent>
              ))}
            </div>
          </Tabs>
        )}
      </div>
      <p className="sr-only" aria-live="polite">
        {searchAnnouncement}
      </p>
    </div>
  );
}
