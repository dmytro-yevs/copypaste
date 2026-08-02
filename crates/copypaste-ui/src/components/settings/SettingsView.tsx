import {
  CircleHelp,
  Database,
  Keyboard,
  List,
  MonitorSmartphone,
  ClipboardCheck,
  Palette,
  Settings2,
  Stethoscope,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useDeferredValue, useEffect, useMemo, useRef, useState } from "react";

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useTranslation } from "@/i18n";
import { AboutTab } from "@/components/settings/AboutTab";
import { AppearanceTab } from "@/components/settings/AppearanceTab";
import { DiagnosticsTab } from "@/components/settings/DiagnosticsTab";
import { ListTab } from "@/components/settings/ListTab";
import { ServiceTab } from "@/components/settings/ServiceTab";
import { ShortcutTab } from "@/components/settings/ShortcutTab";
import { StorageTab } from "@/components/settings/StorageTab";
import { SyncTab } from "@/components/settings/SyncTab";
import { CaptureSetup } from "@/components/capture/CaptureSetup";
import {
  resolveSettingsSearch,
  SettingsSearchField,
  type ResolvedSettingsSearchItem,
} from "@/components/settings/SettingsSearch";
import {
  SETTINGS_SEARCH_ITEMS,
  type SettingsSearchTab,
} from "@/components/settings/settingsSearchIndex";
import { cn } from "@/lib/cn";
import { isAndroid } from "@/lib/platform";
import { useUi } from "@/store/ui";

const TABS = [
  { value: "appearance", label: "settings.tabs.appearance", icon: Palette, render: () => <AppearanceTab /> },
  { value: "list", label: "settings.tabs.list", icon: List, render: () => <ListTab /> },
  { value: "shortcut", label: "settings.tabs.shortcut", icon: Keyboard, render: () => <ShortcutTab /> },
  { value: "service", label: "settings.tabs.service", icon: Settings2, render: () => <ServiceTab /> },
  { value: "capture", label: "capture.title", icon: ClipboardCheck, render: () => <CaptureSetup /> },
  { value: "sync", label: "settings.tabs.sync", icon: MonitorSmartphone, render: () => <SyncTab /> },
  { value: "storage", label: "settings.tabs.storage", icon: Database, render: () => <StorageTab /> },
  { value: "diagnostics", label: "settings.tabs.diagnostics", icon: Stethoscope, render: () => <DiagnosticsTab /> },
  { value: "about", label: "settings.tabs.about", icon: CircleHelp, render: () => <AboutTab /> },
] as const;

type SettingsTab = (typeof TABS)[number];

const GROUPS = [
  { label: "settings.groups.personal", tabs: ["appearance", "list", "shortcut"] },
  { label: "settings.groups.service", tabs: ["service", "sync", "storage"] },
  { label: "settings.groups.support", tabs: ["diagnostics", "about"] },
] as const satisfies ReadonlyArray<{
  label: "settings.groups.personal" | "settings.groups.service" | "settings.groups.support";
  tabs: readonly SettingsTab["value"][];
}>;

function TabButton({ tab, desktop }: { tab: SettingsTab; desktop: boolean }) {
  const { t } = useTranslation();
  const Icon: LucideIcon = tab.icon;

  return (
    <TabsTrigger
      value={tab.value}
      className={cn(
        desktop && "w-full justify-start rounded-lg px-s-2 text-left data-[state=active]:bg-muted data-[state=active]:shadow-none",
      )}
    >
      {desktop && <Icon aria-hidden="true" />}
      {t(tab.label)}
    </TabsTrigger>
  );
}

export function SettingsView() {
  const { t } = useTranslation();
  const android = isAndroid();
  const [activeTab, setActiveTab] = useState<SettingsSearchTab>("appearance");
  const requestedTab = useUi((state) => state.settingsTab);
  const setSettingsTab = useUi((state) => state.setSettingsTab);
  const [query, setQuery] = useState("");
  const [searchAnnouncement, setSearchAnnouncement] = useState("");
  const highlightTimer = useRef<number | undefined>(undefined);
  const deferredQuery = useDeferredValue(query);
  // Android has its own capture setup, but the in-process backend persists
  // service limits and source exclusions too. Keep the Service tab visible so
  // those controls do not silently disappear on a phone.
  const tabs = android
    ? TABS.filter(
        (tab) =>
          tab.value !== "shortcut" && tab.value !== "storage",
      )
    : TABS.filter((tab) => tab.value !== "capture");
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
            (!item.platforms || item.platforms.includes(android ? "android" : "desktop")),
        ),
        tabLabels,
        (key) => t(key as never),
        deferredQuery,
      ),
    [deferredQuery, t, tabLabels],
  );
  useEffect(() => {
    if (requestedTab === null) return;
    if (tabs.some((tab) => tab.value === requestedTab)) {
      setActiveTab(requestedTab as SettingsSearchTab);
    }
    setSettingsTab(null);
  }, [requestedTab, setSettingsTab, tabs]);

  useEffect(
    () => () => {
      if (highlightTimer.current !== undefined) {
        window.clearTimeout(highlightTimer.current);
      }
    },
    [],
  );

  const selectResult = (result: ResolvedSettingsSearchItem) => {
    setActiveTab(result.item.tab);
    setQuery("");
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
      <header
        aria-labelledby="settings-page-title"
        className="chrome flex w-full shrink-0 flex-wrap items-center gap-s-2 border-b border-divider px-s-3 py-s-2 sm:px-s-4"
      >
        <h1 id="settings-page-title" className="sr-only">
          {t("settings.title")}
        </h1>
        <SettingsSearchField
          query={query}
          onQueryChange={setQuery}
          results={results}
          onSelect={selectResult}
        />
      </header>

      <div className={cn("min-h-0 flex-1 overflow-y-auto", android && "p-s-3")}>
        <Tabs
          value={activeTab}
          onValueChange={(value) => setActiveTab(value as SettingsSearchTab)}
          orientation={android ? "horizontal" : "vertical"}
          className={cn(android ? "" : "min-h-full flex-row gap-0")}
        >
          <TabsList
            aria-label={t("settings.sections")}
            variant={android ? "floating" : "bare"}
            className={cn(
              android
                ? "w-full"
                : "min-h-full w-60 shrink-0 self-stretch flex-col items-stretch gap-s-4 border-r border-divider px-s-3 py-s-4 text-foreground",
            )}
          >
              {android
                ? tabs.map((tab) => <TabButton key={tab.value} tab={tab} desktop={false} />)
                : GROUPS.map((group) => (
                    <div key={group.label} className="flex flex-col gap-1">
                      <p className="pt-s-2 text-[11px] font-medium tracking-wide text-muted-foreground">
                        {t(group.label)}
                      </p>
                      {group.tabs.map((value) => {
                        const tab = TABS.find((candidate) => candidate.value === value)!;
                        return <TabButton key={tab.value} tab={tab} desktop />;
                      })}
                    </div>
                  ))}
          </TabsList>

          <div className={cn("min-h-full min-w-0 flex-1", !android && "px-s-4 py-s-3 sm:px-s-5")}>
            {tabs.map((tab) => (
              <TabsContent key={tab.value} value={tab.value} className="min-h-full">
                {tab.render()}
              </TabsContent>
            ))}
          </div>
        </Tabs>
      </div>
      <p className="sr-only" aria-live="polite">
        {searchAnnouncement}
      </p>
    </div>
  );
}
