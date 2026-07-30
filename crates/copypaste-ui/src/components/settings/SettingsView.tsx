/**
 * Sub-navigation is Radix Tabs: A11Y-6 (roles, arrow keys, Home/End,
 * wrap-around) comes with it. A11Y-15: the tab row **wraps** — at the 720px
 * minimum v1's overflowed behind a scrollbar-less scroller and its last tab was
 * entirely off-screen (CopyPaste-g27b.31).
 *
 * Everything here except About and Sync is client-owned, so Settings stays
 * usable while the service is down.
 */
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useTranslation } from "@/i18n";
import { AboutTab } from "@/components/settings/AboutTab";
import { AppearanceTab } from "@/components/settings/AppearanceTab";
import { ListTab } from "@/components/settings/ListTab";
import { ServiceTab } from "@/components/settings/ServiceTab";
import { ShortcutTab } from "@/components/settings/ShortcutTab";
import { StorageTab } from "@/components/settings/StorageTab";
import { SyncTab } from "@/components/settings/SyncTab";

const TABS = [
  { value: "appearance", label: "settings.tabs.appearance", render: () => <AppearanceTab /> },
  { value: "list", label: "settings.tabs.list", render: () => <ListTab /> },
  { value: "shortcut", label: "settings.tabs.shortcut", render: () => <ShortcutTab /> },
  { value: "service", label: "settings.tabs.service", render: () => <ServiceTab /> },
  { value: "sync", label: "settings.tabs.sync", render: () => <SyncTab /> },
  { value: "storage", label: "settings.tabs.storage", render: () => <StorageTab /> },
  { value: "about", label: "settings.tabs.about", render: () => <AboutTab /> },
] as const;

export function SettingsView() {
  const { t } = useTranslation();

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex shrink-0 items-center border-b border-divider bg-panel px-s-3 py-s-2">
        <h1 className="text-sm font-semibold">{t("settings.title")}</h1>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto p-s-3">
        <div className="mx-auto flex max-w-[var(--content-max-width)] flex-col gap-s-3">
          <Tabs defaultValue="appearance">
            <TabsList aria-label={t("settings.sections")}>
              {TABS.map((tab) => (
                <TabsTrigger key={tab.value} value={tab.value}>
                  {t(tab.label)}
                </TabsTrigger>
              ))}
            </TabsList>

            {TABS.map((tab) => (
              <TabsContent key={tab.value} value={tab.value}>
                {tab.render()}
              </TabsContent>
            ))}
          </Tabs>
        </div>
      </div>
    </div>
  );
}
