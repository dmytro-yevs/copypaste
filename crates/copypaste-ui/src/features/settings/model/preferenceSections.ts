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
  | "runtime-events"
  | "about";

export interface PreferenceSectionDefinition {
  readonly value: PreferenceSection;
  readonly label: string;
  readonly description: string;
  readonly icon: SettingsTabIconName;
  readonly capability?: Exclude<keyof SettingsCapabilities, "platform">;
}

export const PREFERENCE_SECTIONS: readonly PreferenceSectionDefinition[] = [
  {
    value: "appearance",
    label: "Appearance",
    description: "Light, dark, color theme and translucency",
    icon: "palette",
  },
  {
    value: "clipboard",
    label: "Clipboard behavior",
    description: "Capture, duplicate and paste rules",
    icon: "capture",
  },
  {
    value: "privacy",
    label: "Privacy & retention",
    description: "Private mode, sensitive content and retention",
    icon: "service",
  },
  {
    value: "shortcuts",
    label: "Shortcuts",
    description: "Quick Paste shortcut and startup",
    icon: "keyboard",
    capability: "shortcut",
  },
  {
    value: "device-sync",
    label: "Device sync",
    description: "This device, nearby devices and network access",
    icon: "devices",
  },
  {
    value: "cloud-sync",
    label: "Cloud sync",
    description: "Account, encryption and cloud status",
    icon: "cloud",
  },
  {
    value: "storage",
    label: "Storage & history",
    description: "Stored items, cleanup, transfer and recovery",
    icon: "storage",
  },
  {
    value: "diagnostics",
    label: "Diagnostics",
    description: "Service state and support report",
    icon: "diagnostics",
  },
  {
    value: "runtime-events",
    label: "Runtime events",
    description: "Search and inspect service activity",
    icon: "list",
  },
  {
    value: "about",
    label: "About",
    description: "Versions, links and product information",
    icon: "help",
  },
];

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
      return "runtime-events";
    case "about":
      return "about";
    default:
      return "appearance";
  }
}
