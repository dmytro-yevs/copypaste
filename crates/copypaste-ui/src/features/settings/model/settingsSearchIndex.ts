import type {
  SettingsCapabilities,
} from "./settingsNavigation";
import type { PreferenceSection } from "./preferenceSections";

export type SettingsSearchTab = PreferenceSection;

export interface SettingsSearchItem {
  tab: SettingsSearchTab;
  section?: string;
  title: string;
  description?: string;
  keywords?: readonly string[];
  platforms?: readonly ("desktop" | "android" | "windows")[];
  capability?: Exclude<keyof SettingsCapabilities, "platform">;
}

/** Every settings row is listed here so search does not depend on hidden tabs
 * being mounted. Translation keys keep the index correct when copy changes. */
export const SETTINGS_SEARCH_ITEMS: readonly SettingsSearchItem[] = [
  { tab: "appearance", title: "settings.appearance.theme.title", description: "settings.appearance.theme.override", keywords: ["dark", "light", "system"] },
  { tab: "appearance", title: "settings.appearance.colorTheme.title", description: "settings.appearance.colorTheme.description", keywords: ["colour", "color", "midnight", "aurora", "ember", "graphite"] },
  { tab: "appearance", title: "settings.appearance.translucency.title", description: "settings.appearance.translucency.description", keywords: ["transparency", "frost"], capability: "translucency" },

  { tab: "clipboard", title: "settings.list.groupByDevice.title", description: "settings.list.groupByDevice.description" },
  { tab: "clipboard", title: "settings.list.historyDisplayLimit.title", description: "settings.list.historyDisplayLimit.description", keywords: ["items", "limit"] },
  { tab: "privacy", title: "settings.list.warnBeforeReveal.title", description: "settings.list.warnBeforeReveal.description", keywords: ["password", "secret", "token"] },
  { tab: "privacy", title: "settings.list.allowScreenshots.title", description: "settings.list.allowScreenshots.description", keywords: ["screen recording", "privacy"], capability: "screenshots" },

  { tab: "shortcuts", title: "settings.shortcut.title", description: "settings.shortcut.description", keywords: ["hotkey", "keyboard", "quick paste"] },
  { tab: "shortcuts", section: "settings.startup.title", title: "settings.startup.openAtLogin.title", description: "settings.startup.openAtLogin.description", keywords: ["autostart", "login", "sign in", "boot", "launch", "start with windows", "login items"], capability: "startup" },

  { tab: "clipboard", title: "capture.title", description: "capture.loading.body", keywords: ["background", "clipboard", "recording", "paused"] },
  { tab: "privacy", title: "settings.service.privateMode.title", description: "settings.service.privateMode.description" },
  { tab: "clipboard", section: "settings.service.groups.capture.title", title: "settings.service.poll.title", description: "settings.service.poll.description" },
  { tab: "clipboard", section: "settings.service.groups.capture.title", title: "settings.service.dedup.title", description: "settings.service.dedup.description" },
  { tab: "clipboard", section: "settings.service.groups.capture.title", title: "settings.service.maxText.title", description: "settings.service.maxText.description" },
  { tab: "clipboard", section: "settings.service.groups.capture.title", title: "settings.service.maxImage.title", description: "settings.service.maxImage.description" },
  { tab: "clipboard", section: "settings.service.groups.capture.title", title: "settings.service.maxFile.title", description: "settings.service.maxFile.description" },
  { tab: "clipboard", section: "settings.service.groups.capture.title", title: "settings.service.maxDecodedImage.title", description: "settings.service.maxDecodedImage.description" },
  { tab: "clipboard", section: "settings.service.groups.capture.title", title: "settings.service.exclusions.title", description: "settings.service.exclusions.description", keywords: ["app", "application", "exclude", "source", "bundle", "package", "privacy", "program", "exe"] },
  { tab: "privacy", section: "settings.service.groups.keeping.title", title: "settings.service.historyLimit.title", description: "settings.service.historyLimit.description" },
  { tab: "privacy", section: "settings.service.groups.keeping.title", title: "settings.service.storageQuota.title", description: "settings.service.storageQuota.description" },
  { tab: "privacy", section: "settings.service.groups.keeping.title", title: "settings.service.retention.title", description: "settings.service.retention.description" },
  { tab: "privacy", section: "settings.service.groups.keeping.title", title: "settings.service.sensitive.title", description: "settings.service.sensitive.description", keywords: ["password", "key", "token", "delete"] },
  { tab: "clipboard", section: "settings.service.groups.telling.title", title: "settings.service.notify.title", description: "settings.service.notify.description", keywords: ["notification"] },
  { tab: "clipboard", section: "settings.service.groups.telling.title", title: "settings.service.sound.title", description: "settings.service.sound.description" },
  { tab: "device-sync", section: "settings.service.groups.network.title", title: "settings.service.syncEnabled.title", description: "settings.service.syncEnabled.description", keywords: ["pair", "devices"] },
  { tab: "device-sync", section: "settings.service.groups.network.title", title: "settings.service.lan.title", description: "settings.service.lan.description", keywords: ["network", "discover"] },

  { tab: "clipboard", title: "capture.setup.enable.title", description: "capture.setup.enable.body", keywords: ["android", "shizuku", "permission", "clipboard"], platforms: ["android"] },
  { tab: "clipboard", title: "settings.service.exclusions.title", description: "settings.service.exclusions.androidLimitation", keywords: ["app", "application", "exclude", "source", "package", "privacy"], platforms: ["android"] },
  { tab: "clipboard", section: "capture.setup.always.title", title: "capture.setup.always.action", description: "capture.setup.always.body", platforms: ["android"] },
  { tab: "clipboard", section: "capture.setup.ladder.title", title: "capture.setup.ladder.armed", keywords: ["other apps", "shizuku", "permission"], platforms: ["android"] },
  { tab: "clipboard", title: "capture.toast.row.title", description: "capture.toast.row.body", keywords: ["android", "notice"], platforms: ["android"] },

  { tab: "device-sync", title: "devices.own.rename.label", description: "devices.own.rename.description", keywords: ["device name", "rename", "this device"] },
  { tab: "device-sync", title: "settings.sync.paired.title", description: "settings.sync.paired.description", keywords: ["pair", "devices", "encrypted"] },
  { tab: "device-sync", title: "settings.sync.now.title", description: "settings.sync.now.description" },
  { tab: "cloud-sync", title: "settings.sync.cloud.title", description: "settings.sync.cloud.description", keywords: ["account", "supabase", "internet"] },

  { tab: "storage", title: "settings.storage.stored.title", description: "settings.storage.stored.description" },
  { tab: "storage", title: "settings.transfer.export.title", description: "settings.transfer.export.description" },
  { tab: "storage", title: "settings.transfer.import.title", description: "settings.transfer.import.description" },
  { tab: "storage", section: "settings.transfer.recoverySection", title: "settings.transfer.backup.title", description: "settings.transfer.backup.description" },
  { tab: "storage", section: "settings.transfer.recoverySection", title: "settings.transfer.restore.title", description: "settings.transfer.restore.description" },
  { tab: "storage", title: "settings.storage.clear.title", description: "settings.storage.clear.description" },

  { tab: "diagnostics", section: "settings.diagnostics.running.title", title: "settings.diagnostics.running.history.title", description: "settings.diagnostics.running.history.description" },
  { tab: "diagnostics", section: "settings.diagnostics.running.title", title: "settings.diagnostics.running.started.title", description: "settings.diagnostics.running.started.description" },
  { tab: "diagnostics", section: "settings.diagnostics.dropped.title", title: "settings.diagnostics.dropped.tooLarge.title", description: "settings.diagnostics.dropped.tooLarge.description" },
  { tab: "diagnostics", section: "settings.diagnostics.dropped.title", title: "settings.diagnostics.dropped.missed.title", description: "settings.diagnostics.dropped.missed.description" },
  { tab: "diagnostics", section: "settings.diagnostics.dropped.title", title: "settings.diagnostics.dropped.swept.title", description: "settings.diagnostics.dropped.swept.description" },
  { tab: "diagnostics", section: "settings.diagnostics.dropped.title", title: "settings.diagnostics.dropped.purged.title", description: "settings.diagnostics.dropped.purged.description" },
  { tab: "diagnostics", title: "settings.diagnostics.report.title", keywords: ["copy", "export", "logs", "support"] },

  { tab: "runtime-events", title: "runtimeLog.title", keywords: ["logs", "events", "service", "activity"] },

  { tab: "about", title: "settings.about.app.title", description: "settings.about.app.description" },
  { tab: "about", title: "settings.about.updates.title", description: "settings.about.updates.description", keywords: ["update", "upgrade", "version"] },
  { tab: "about", title: "settings.about.service.title" },
  { tab: "about", title: "settings.about.capture.title", description: "settings.about.capture.description" },
  { tab: "about", title: "settings.about.backend.title", description: "settings.about.backend.description" },
  { tab: "about", title: "settings.about.protocol.title", description: "settings.about.protocol.description" },
  { tab: "about", title: "settings.about.items.title", description: "settings.about.items.description" },
  { tab: "about", title: "settings.about.links.title" },
  { tab: "about", title: "onboarding.settings.title", description: "onboarding.settings.description", keywords: ["setup", "onboarding", "welcome", "first run"] },
  { tab: "about", title: "settings.about.reset.title", description: "settings.about.reset.description", keywords: ["defaults", "restore"] },
];
