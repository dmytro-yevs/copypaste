import type { AppPlatform } from "@/lib/platform";

export type SettingsTabValue =
  | "appearance"
  | "list"
  | "shortcut"
  | "service"
  | "capture"
  | "sync"
  | "storage"
  | "diagnostics"
  | "about";

export type SettingsTabIconName =
  | "cloud"
  | "palette"
  | "list"
  | "keyboard"
  | "service"
  | "capture"
  | "devices"
  | "storage"
  | "diagnostics"
  | "help";

type SettingsTabLabel =
  | "settings.tabs.appearance"
  | "settings.tabs.list"
  | "settings.tabs.shortcut"
  | "settings.tabs.service"
  | "capture.title"
  | "settings.tabs.sync"
  | "settings.tabs.storage"
  | "settings.tabs.diagnostics"
  | "settings.tabs.about";

export interface SettingsTab {
  readonly value: SettingsTabValue;
  readonly label: SettingsTabLabel;
  readonly icon: SettingsTabIconName;
}

export type SettingsGroupLabel =
  | "settings.groups.personal"
  | "settings.groups.service"
  | "settings.groups.support";

export interface SettingsGroup {
  readonly label: SettingsGroupLabel;
  readonly tabs: readonly SettingsTab[];
}

export interface SettingsCapabilities {
  readonly platform: AppPlatform;
  readonly shortcut: boolean;
  readonly startup: boolean;
  readonly androidCapture: boolean;
  readonly translucency: boolean;
  readonly screenshots: boolean;
  readonly updater: boolean;
}

export function settingsCapabilities(platform: AppPlatform): SettingsCapabilities {
  const desktopNative = platform === "macos" || platform === "windows";
  return {
    platform,
    shortcut: desktopNative || platform === "browser",
    startup: desktopNative,
    androidCapture: platform === "android",
    translucency: desktopNative,
    screenshots: desktopNative || platform === "android",
    updater: desktopNative || platform === "android",
  };
}
