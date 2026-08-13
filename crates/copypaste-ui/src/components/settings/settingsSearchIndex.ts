export type SettingsSearchTab =
  | "appearance"
  | "list"
  | "shortcut"
  | "service"
  | "capture"
  | "sync"
  | "storage"
  | "diagnostics"
  | "about";

export interface SettingsSearchItem {
  tab: SettingsSearchTab;
  section?: string;
  title: string;
  description?: string;
  keywords?: readonly string[];
  platforms?: readonly ("desktop" | "android" | "windows")[];
}

/** Every settings row is listed here so search does not depend on hidden tabs
 * being mounted. Translation keys keep the index correct when copy changes. */
export const SETTINGS_SEARCH_ITEMS: readonly SettingsSearchItem[] = [
  { tab: "appearance", title: "settings.appearance.theme.title", description: "settings.appearance.theme.override", keywords: ["dark", "light", "system"] },
  { tab: "appearance", title: "settings.appearance.accent.title", description: "settings.appearance.accent.description", keywords: ["colour", "color", "system", "dynamic", "indigo", "blue", "teal", "green", "amber", "rose"] },
  { tab: "appearance", title: "settings.appearance.translucency.title", description: "settings.appearance.translucency.description", keywords: ["transparency", "frost"], platforms: ["desktop"] },

  { tab: "list", title: "settings.list.previewLines.title", description: "settings.list.previewLines.description" },
  { tab: "list", title: "settings.list.groupByDevice.title", description: "settings.list.groupByDevice.description" },
  { tab: "list", title: "settings.list.popupPreviewLines.title", description: "settings.list.popupPreviewLines.description", keywords: ["quick paste"] },
  { tab: "list", title: "settings.list.historyDisplayLimit.title", description: "settings.list.historyDisplayLimit.description", keywords: ["items", "limit"] },
  { tab: "list", title: "settings.list.warnBeforeReveal.title", description: "settings.list.warnBeforeReveal.description", keywords: ["password", "secret", "token"] },
  { tab: "list", title: "settings.list.allowScreenshots.title", description: "settings.list.allowScreenshots.description", keywords: ["screen recording", "privacy"] },

  { tab: "shortcut", title: "settings.shortcut.title", description: "settings.shortcut.description", keywords: ["hotkey", "keyboard", "quick paste"] },
  { tab: "shortcut", section: "settings.startup.title", title: "settings.startup.openAtLogin.title", description: "settings.startup.openAtLogin.description", keywords: ["autostart", "login", "sign in", "boot", "launch", "start with windows", "login items"], platforms: ["desktop"] },

  { tab: "service", section: "settings.service.groups.capture.title", title: "settings.service.privateMode.title", description: "settings.service.privateMode.description" },
  { tab: "service", section: "settings.service.groups.capture.title", title: "settings.service.poll.title", description: "settings.service.poll.description" },
  { tab: "service", section: "settings.service.groups.capture.title", title: "settings.service.dedup.title", description: "settings.service.dedup.description" },
  { tab: "service", section: "settings.service.groups.capture.title", title: "settings.service.maxText.title", description: "settings.service.maxText.description" },
  { tab: "service", section: "settings.service.groups.capture.title", title: "settings.service.maxImage.title", description: "settings.service.maxImage.description" },
  { tab: "service", section: "settings.service.groups.capture.title", title: "settings.service.maxFile.title", description: "settings.service.maxFile.description" },
  { tab: "service", section: "settings.service.groups.capture.title", title: "settings.service.maxDecodedImage.title", description: "settings.service.maxDecodedImage.description" },
  { tab: "service", section: "settings.service.groups.capture.title", title: "settings.service.exclusions.title", description: "settings.service.exclusions.description", keywords: ["app", "application", "exclude", "source", "bundle", "package", "privacy", "program", "exe"] },
  { tab: "service", section: "settings.service.groups.keeping.title", title: "settings.service.historyLimit.title", description: "settings.service.historyLimit.description" },
  { tab: "service", section: "settings.service.groups.keeping.title", title: "settings.service.storageQuota.title", description: "settings.service.storageQuota.description" },
  { tab: "service", section: "settings.service.groups.keeping.title", title: "settings.service.retention.title", description: "settings.service.retention.description" },
  { tab: "service", section: "settings.service.groups.keeping.title", title: "settings.service.sensitive.title", description: "settings.service.sensitive.description", keywords: ["password", "key", "token", "delete"] },
  { tab: "service", section: "settings.service.groups.telling.title", title: "settings.service.notify.title", description: "settings.service.notify.description", keywords: ["notification"] },
  { tab: "service", section: "settings.service.groups.telling.title", title: "settings.service.sound.title", description: "settings.service.sound.description" },
  { tab: "service", section: "settings.service.groups.network.title", title: "settings.service.syncEnabled.title", description: "settings.service.syncEnabled.description", keywords: ["pair", "devices"] },
  { tab: "service", section: "settings.service.groups.network.title", title: "settings.service.lan.title", description: "settings.service.lan.description", keywords: ["network", "discover"] },

  { tab: "capture", title: "capture.setup.enable.title", description: "capture.setup.enable.body", keywords: ["android", "shizuku", "permission", "clipboard"] },
  { tab: "capture", title: "settings.service.exclusions.title", description: "settings.service.exclusions.androidLimitation", keywords: ["app", "application", "exclude", "source", "package", "privacy"], platforms: ["android"] },
  { tab: "capture", section: "capture.setup.always.title", title: "capture.setup.always.action", description: "capture.setup.always.body" },
  { tab: "capture", section: "capture.setup.ladder.title", title: "capture.setup.ladder.armed", keywords: ["other apps", "shizuku", "permission"] },
  { tab: "capture", title: "capture.toast.row.title", description: "capture.toast.row.body", keywords: ["android", "notice"] },

  { tab: "sync", title: "settings.sync.paired.title", description: "settings.sync.paired.description", keywords: ["pair", "devices", "encrypted"] },
  { tab: "sync", title: "settings.sync.now.title", description: "settings.sync.now.description" },
  { tab: "sync", title: "settings.sync.cloud.title", description: "settings.sync.cloud.description", keywords: ["account", "supabase", "internet"] },

  { tab: "storage", title: "settings.storage.stored.title", description: "settings.storage.stored.description" },
  { tab: "storage", title: "settings.transfer.export.title", description: "settings.transfer.export.description" },
  { tab: "storage", title: "settings.transfer.import.title", description: "settings.transfer.import.description" },
  { tab: "storage", title: "settings.transfer.backup.title", description: "settings.transfer.backup.description" },
  { tab: "storage", title: "settings.transfer.restore.title", description: "settings.transfer.restore.description" },
  { tab: "storage", title: "settings.storage.clear.title", description: "settings.storage.clear.description" },

  { tab: "diagnostics", section: "settings.diagnostics.running.title", title: "settings.diagnostics.running.service.title" },
  { tab: "diagnostics", section: "settings.diagnostics.running.title", title: "settings.diagnostics.running.capture.title", description: "settings.diagnostics.running.capture.description" },
  { tab: "diagnostics", section: "settings.diagnostics.running.title", title: "settings.diagnostics.running.backend.title", description: "settings.diagnostics.running.backend.description" },
  { tab: "diagnostics", section: "settings.diagnostics.running.title", title: "settings.diagnostics.running.protocol.title", description: "settings.diagnostics.running.protocol.description" },
  { tab: "diagnostics", section: "settings.diagnostics.running.title", title: "settings.diagnostics.running.started.title", description: "settings.diagnostics.running.started.description" },
  { tab: "diagnostics", section: "settings.diagnostics.dropped.title", title: "settings.diagnostics.dropped.tooLarge" },
  { tab: "diagnostics", section: "settings.diagnostics.dropped.title", title: "settings.diagnostics.dropped.missed" },
  { tab: "diagnostics", section: "settings.diagnostics.dropped.title", title: "settings.diagnostics.dropped.swept" },
  { tab: "diagnostics", section: "settings.diagnostics.dropped.title", title: "settings.diagnostics.dropped.purged" },
  { tab: "diagnostics", title: "settings.diagnostics.report.title", keywords: ["copy", "export", "logs", "support"] },

  { tab: "about", title: "settings.about.app.title", description: "settings.about.app.description" },
  { tab: "about", title: "settings.about.updates.title", description: "settings.about.updates.description", keywords: ["update", "upgrade", "version"], platforms: ["windows"] },
  { tab: "about", title: "settings.about.service.title" },
  { tab: "about", title: "settings.about.capture.title", description: "settings.about.capture.description" },
  { tab: "about", title: "settings.about.backend.title", description: "settings.about.backend.description" },
  { tab: "about", title: "settings.about.protocol.title", description: "settings.about.protocol.description" },
  { tab: "about", title: "settings.about.items.title", description: "settings.about.items.description" },
  { tab: "about", title: "settings.about.links.title" },
  { tab: "about", title: "settings.about.reset.title", description: "settings.about.reset.description", keywords: ["defaults", "restore"] },
];
