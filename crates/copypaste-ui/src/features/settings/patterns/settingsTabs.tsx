import type { ReactNode } from "react";

import { CaptureSetupState } from "@/features/capture";
import { AboutTab } from "@/features/settings/patterns/AboutTab";
import { AppearanceTab } from "@/features/settings/patterns/AppearanceTab";
import { CloudSyncSettings } from "@/features/settings/patterns/CloudSyncSettings";
import { DeviceSyncSettings } from "@/features/settings/patterns/DeviceSyncSettings";
import { DiagnosticsTab } from "@/features/settings/patterns/DiagnosticsTab";
import { RuntimeEventsTab } from "@/features/settings/patterns/RuntimeEventsTab";
import {
  ClipboardListSettings,
  ListTab,
  PrivacyDisplaySettings,
} from "@/features/settings/patterns/ListTab";
import {
  AdvancedServiceSettings,
  ClipboardServiceSettings,
  PrivacyServiceSettings,
  ServiceTab,
} from "@/features/settings/patterns/ServiceTab";
import { ShortcutTab } from "@/features/settings/patterns/ShortcutTab";
import { StorageTab } from "@/features/settings/patterns/StorageTab";
import { SyncTab } from "@/features/settings/patterns/SyncTab";
import type {
  SettingsCapabilities,
  SettingsGroup,
  SettingsTab,
} from "@/features/settings/model/settingsNavigation";
import type { PreferenceSection } from "@/features/settings/model/preferenceSections";

export interface SettingsTabController {
  readonly prefsReady: boolean;
  readonly capabilities: SettingsCapabilities;
}

export function renderPreferenceSection(
  section: PreferenceSection,
  controller: SettingsTabController,
): ReactNode {
  switch (section) {
    case "appearance": return (
      <AppearanceTab
        ready={controller.prefsReady}
        supportsTranslucency={controller.capabilities.translucency}
      />
    );
    case "clipboard": return (
      <>
        <ClipboardServiceSettings />
        {controller.capabilities.androidCapture ? (
          <CaptureSetupState mode="supplemental" />
        ) : null}
        <ClipboardListSettings
          ready={controller.prefsReady}
          supportsScreenshots={controller.capabilities.screenshots}
        />
      </>
    );
    case "privacy": return (
      <>
        <PrivacyServiceSettings />
        <PrivacyDisplaySettings
          ready={controller.prefsReady}
          supportsScreenshots={controller.capabilities.screenshots}
        />
      </>
    );
    case "shortcuts": return (
      controller.capabilities.shortcut ? (
        <ShortcutTab supportsStartup={controller.capabilities.startup} />
      ) : null
    );
    case "device-sync": return (
      <>
        <DeviceSyncSettings />
        <AdvancedServiceSettings />
      </>
    );
    case "cloud-sync": return (
      <CloudSyncSettings />
    );
    case "storage": return <StorageTab />;
    case "diagnostics": return <DiagnosticsTab />;
    case "runtime-events": return <RuntimeEventsTab />;
    case "about": return (
      <AboutTab />
    );
  }
}

export const TABS = [
  { value: "appearance", label: "settings.tabs.appearance", icon: "palette" },
  { value: "list", label: "settings.tabs.list", icon: "list" },
  { value: "shortcut", label: "settings.tabs.shortcut", icon: "keyboard" },
  { value: "service", label: "settings.tabs.service", icon: "service" },
  { value: "capture", label: "capture.title", icon: "capture" },
  { value: "sync", label: "settings.tabs.sync", icon: "devices" },
  { value: "storage", label: "settings.tabs.storage", icon: "storage" },
  { value: "diagnostics", label: "settings.tabs.diagnostics", icon: "diagnostics" },
  { value: "about", label: "settings.tabs.about", icon: "help" },
] as const satisfies readonly SettingsTab[];

export function renderSettingsTab(
  value: SettingsTab["value"],
  controller: SettingsTabController,
): ReactNode {
  switch (value) {
    case "appearance": return (
      <AppearanceTab
        ready={controller.prefsReady}
        supportsTranslucency={controller.capabilities.translucency}
      />
    );
    case "list": return (
      <ListTab
        ready={controller.prefsReady}
        supportsScreenshots={controller.capabilities.screenshots}
      />
    );
    case "shortcut": return (
      <ShortcutTab supportsStartup={controller.capabilities.startup} />
    );
    case "service": return <ServiceTab />;
    case "capture": return <CaptureSetupState />;
    case "sync": return <SyncTab />;
    case "storage": return <StorageTab />;
    case "diagnostics": return <DiagnosticsTab />;
    case "about": return (
      <AboutTab />
    );
  }
}

const GROUPS = [
  { label: "settings.groups.personal", tabs: ["appearance", "list", "shortcut"] },
  { label: "settings.groups.service", tabs: ["service", "capture", "sync", "storage"] },
  { label: "settings.groups.support", tabs: ["diagnostics", "about"] },
] as const satisfies ReadonlyArray<{
  label: SettingsGroup["label"];
  tabs: readonly SettingsTab["value"][];
}>;

/** Android has its own capture setup, but Service and Storage expose controls
 *  backed by native commands on every product platform. */
export function visibleTabs(capabilities: SettingsCapabilities): readonly SettingsTab[] {
  return TABS.filter((tab) => {
    if (tab.value === "shortcut") return capabilities.shortcut;
    if (tab.value === "capture") return capabilities.androidCapture;
    return true;
  });
}

/** Grouping follows what is visible, so a group whose whole membership is
 *  absent on this platform does not render an empty heading. */
export function groupedTabs(tabs: readonly SettingsTab[]): readonly SettingsGroup[] {
  return GROUPS.map((group) => ({
    label: group.label,
    tabs: group.tabs.flatMap((value) => tabs.filter((tab) => tab.value === value)),
  })).filter((group) => group.tabs.length > 0);
}
