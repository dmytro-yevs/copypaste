import type {
  SettingsCapabilities,
  SettingsTabIconName,
  SettingsTabValue,
} from "@/features/settings/model/settingsNavigation";

export type PreferenceSection =
  | "appearance"
  | "clipboard"
  | "privacy"
  | "shortcuts"
  | "device-sync"
  | "cloud-sync"
  | "storage"
  | "diagnostics"
  | "about";

export interface PreferenceSectionDefinition {
  readonly value: PreferenceSection;
  readonly label: string;
  readonly description: string;
  readonly icon: SettingsTabIconName;
  readonly capability?: Exclude<keyof SettingsCapabilities, "platform">;
}

import { SETTINGS_SECTIONS } from "./settingsSchemaCatalog";

export const PREFERENCE_SECTIONS: readonly PreferenceSectionDefinition[] = SETTINGS_SECTIONS;

export function visiblePreferenceSections(
  capabilities: SettingsCapabilities,
): readonly PreferenceSectionDefinition[] {
  return PREFERENCE_SECTIONS.filter(
    (section) => !section.capability || capabilities[section.capability],
  );
}

export function preferenceSectionForTab(tab: SettingsTabValue | string): PreferenceSection {
  switch (tab) {
    case "appearance":
      return "appearance";
    case "capture":
    case "clipboard":
    case "list":
      return "clipboard";
    case "privacy":
    case "service":
      return "privacy";
    case "shortcut":
    case "shortcuts":
      return "shortcuts";
    case "device-sync":
    case "sync":
      return "device-sync";
    case "cloud-sync":
      return "cloud-sync";
    case "data-transfer":
    case "transfer":
    case "storage":
      return "storage";
    case "diagnostics":
      return "diagnostics";
    case "runtime-events":
      return "diagnostics";
    case "about":
      return "about";
    default:
      return "appearance";
  }
}
