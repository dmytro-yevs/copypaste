import {
  CircleHelp,
  Database,
  Keyboard,
  List,
  MonitorSmartphone,
  Palette,
  Settings2,
  Stethoscope,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

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
import { cn } from "@/lib/cn";
import { isAndroid } from "@/lib/platform";

const TABS = [
  { value: "appearance", label: "settings.tabs.appearance", icon: Palette, render: () => <AppearanceTab /> },
  { value: "list", label: "settings.tabs.list", icon: List, render: () => <ListTab /> },
  { value: "shortcut", label: "settings.tabs.shortcut", icon: Keyboard, render: () => <ShortcutTab /> },
  { value: "service", label: "settings.tabs.service", icon: Settings2, render: () => <ServiceTab /> },
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
        desktop && "w-full justify-start px-s-2 text-left data-[state=active]:bg-muted data-[state=active]:shadow-none",
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
  // The embedded Android backend has no persistent service configuration or
  // backup/restore. Do not offer controls that can only refuse later.
  const tabs = android
    ? TABS.filter(
        (tab) =>
          tab.value !== "shortcut" && tab.value !== "service" && tab.value !== "storage",
      )
    : TABS;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex shrink-0 items-center border-b border-divider bg-panel px-s-3 py-s-2 sm:px-s-4">
        <h1 className="text-base font-semibold">{t("settings.title")}</h1>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto p-s-3">
        <div className="mx-auto min-h-full max-w-[var(--content-max-width)]">
          <Tabs
            defaultValue="appearance"
            orientation={android ? "horizontal" : "vertical"}
            className={cn(android ? "" : "min-h-full flex-row gap-s-5")}
          >
            <TabsList
              aria-label={t("settings.sections")}
              className={cn(
                android
                  ? "w-full"
                  : "h-fit w-52 shrink-0 flex-col items-stretch gap-s-3 rounded-none bg-transparent p-0 text-foreground",
              )}
            >
              {android
                ? tabs.map((tab) => <TabButton key={tab.value} tab={tab} desktop={false} />)
                : GROUPS.map((group) => (
                    <div key={group.label} className="flex flex-col gap-1">
                      <p className="px-s-2 pt-s-1 text-xs font-medium text-muted-foreground">
                        {t(group.label)}
                      </p>
                      {group.tabs.map((value) => {
                        const tab = TABS.find((candidate) => candidate.value === value)!;
                        return <TabButton key={tab.value} tab={tab} desktop />;
                      })}
                    </div>
                  ))}
            </TabsList>

            <div className="min-w-0 flex-1">
              {tabs.map((tab) => (
                <TabsContent key={tab.value} value={tab.value}>
                  {tab.render()}
                </TabsContent>
              ))}
            </div>
          </Tabs>
        </div>
      </div>
    </div>
  );
}
