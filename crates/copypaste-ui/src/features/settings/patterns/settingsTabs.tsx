import type { ReactNode } from "react";
import { AboutTab } from "@/features/settings/patterns/AboutTab";
import { AppearanceTab } from "@/features/settings/patterns/AppearanceTab";
import { CloudSyncSettings } from "@/features/settings/patterns/CloudSyncSettings";
import { DeviceSyncSettings } from "@/features/settings/patterns/DeviceSyncSettings";
import { DiagnosticsTab, type DiagnosticsView } from "@/features/settings/patterns/DiagnosticsTab";
import { ListTab } from "@/features/settings/patterns/ListTab";
import { ServiceSettings } from "@/features/settings/patterns/ServiceTab";
import { ShortcutTab } from "@/features/settings/patterns/ShortcutTab";
import { StorageTab } from "@/features/settings/patterns/StorageTab";
import type { SettingsCapabilities } from "@/features/settings/model/settingsNavigation";
import type { PreferenceSection } from "@/features/settings/model/preferenceSections";
import { disclosureRevealKey, type SettingsDisclosureReveal } from "@/features/settings/model/settingsSearchIndex";

export interface SettingsSectionController {
  readonly prefsReady: boolean;
  readonly capabilities: SettingsCapabilities;
  readonly disclosureReveal?: SettingsDisclosureReveal;
  readonly diagnosticsView?: DiagnosticsView;
  readonly onOpenEvents?: () => void;
  readonly onBackFromEvents?: () => void;
}

/** The only section dispatcher. Each owner supplies live fields to the shared
 * schema renderer; no legacy tab model or parallel search map is retained. */
export function renderPreferenceSection(section: PreferenceSection, controller: SettingsSectionController): ReactNode {
  switch (section) {
    case "appearance": return <AppearanceTab ready={controller.prefsReady} supportsTranslucency={controller.capabilities.translucency} />;
    case "clipboard": return <><ServiceSettings scope="clipboard" revealAdvancedKey={disclosureRevealKey(controller.disclosureReveal, "clipboard-advanced")} /><ListTab scope="clipboard" ready={controller.prefsReady} supportsScreenshots={controller.capabilities.screenshots} /></>;
    case "privacy": return <><ServiceSettings scope="privacy" /><ListTab scope="privacy" ready={controller.prefsReady} supportsScreenshots={controller.capabilities.screenshots} /></>;
    case "shortcuts": return controller.capabilities.shortcut ? <ShortcutTab supportsStartup={controller.capabilities.startup} /> : null;
    case "device-sync": return <><DeviceSyncSettings /><ServiceSettings scope="device-sync" /></>;
    case "cloud-sync": return <CloudSyncSettings revealAdvancedKey={disclosureRevealKey(controller.disclosureReveal, "cloud-server")} />;
    case "storage": return <StorageTab />;
    case "diagnostics": return <DiagnosticsTab view={controller.diagnosticsView} onOpenEvents={controller.onOpenEvents} onBack={controller.onBackFromEvents} />;
    case "about": return <AboutTab />;
  }
}
