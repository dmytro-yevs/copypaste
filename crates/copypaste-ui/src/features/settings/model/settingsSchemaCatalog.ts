import type { PreferenceSectionDefinition } from "./preferenceSections";
import type { SettingsFieldDefinition, SettingsPlatform } from "./settingsFieldSchema";

/** The canonical field inventory for the nine settings sections. Live values and
 * effects are supplied by feature controllers at render time. */
const SEARCHABLE_FIELDS: readonly Omit<SettingsFieldDefinition, "id">[] = [
  { kind: "choice", section: "appearance", title: "settings.appearance.theme.title", description: "settings.appearance.theme.override", keywords: ["dark", "light", "system"] },
  { kind: "choice", section: "appearance", title: "settings.appearance.colorTheme.title", description: "settings.appearance.colorTheme.description", keywords: ["colour", "color", "midnight", "aurora", "ember", "graphite"] },
  { kind: "boolean", section: "appearance", title: "settings.appearance.translucency.title", description: "settings.appearance.translucency.description", keywords: ["transparency", "frost"], capability: "translucency" },

  { kind: "boolean", section: "clipboard", title: "settings.list.groupByDevice.title", description: "settings.list.groupByDevice.description" },
  { kind: "number", section: "clipboard", title: "settings.list.historyDisplayLimit.title", description: "settings.list.historyDisplayLimit.description", keywords: ["items", "limit"] },
  { kind: "boolean", section: "privacy", title: "settings.list.warnBeforeReveal.title", description: "settings.list.warnBeforeReveal.description", keywords: ["password", "secret", "token"] },
  { kind: "boolean", section: "privacy", title: "settings.list.allowScreenshots.title", description: "settings.list.allowScreenshots.description", keywords: ["screen recording", "privacy"], capability: "screenshots" },

  { kind: "custom", section: "shortcuts", title: "settings.shortcut.title", description: "settings.shortcut.description", keywords: ["hotkey", "keyboard", "quick paste"] },
  { kind: "boolean", section: "shortcuts", group: "settings.startup.title", title: "settings.startup.openAtLogin.title", description: "settings.startup.openAtLogin.description", keywords: ["autostart", "login", "sign in", "boot", "launch", "start with windows", "login items"], capability: "startup" },

  { kind: "custom", section: "clipboard", title: "capture.title", description: "capture.loading.body", keywords: ["background", "clipboard", "recording", "paused"] },
  { kind: "boolean", section: "privacy", title: "settings.service.privateMode.title", description: "settings.service.privateMode.description" },
  { kind: "choice", section: "clipboard", group: "settings.service.advanced.title", title: "settings.service.poll.title", description: "settings.service.poll.description", keywords: ["polling", "interval", "frequency"], disclosure: "clipboard-advanced" },
  { kind: "choice", section: "clipboard", group: "settings.service.groups.capture.title", title: "settings.service.dedup.title", description: "settings.service.dedup.description" },
  { kind: "choice", section: "clipboard", group: "settings.service.advanced.title", title: "settings.service.maxText.title", description: "settings.service.maxText.description", disclosure: "clipboard-advanced" },
  { kind: "choice", section: "clipboard", group: "settings.service.advanced.title", title: "settings.service.maxImage.title", description: "settings.service.maxImage.description", disclosure: "clipboard-advanced" },
  { kind: "choice", section: "clipboard", group: "settings.service.advanced.title", title: "settings.service.maxFile.title", description: "settings.service.maxFile.description", disclosure: "clipboard-advanced" },
  { kind: "choice", section: "clipboard", group: "settings.service.advanced.title", title: "settings.service.maxDecodedImage.title", description: "settings.service.maxDecodedImage.description", disclosure: "clipboard-advanced" },
  { kind: "custom", section: "clipboard", group: "settings.service.groups.capture.title", title: "settings.service.exclusions.title", description: "settings.service.exclusions.description", keywords: ["app", "application", "exclude", "source", "bundle", "package", "privacy", "program", "exe"] },
  { kind: "choice", section: "privacy", group: "settings.service.groups.keeping.title", title: "settings.service.historyLimit.title", description: "settings.service.historyLimit.description" },
  { kind: "choice", section: "privacy", group: "settings.service.groups.keeping.title", title: "settings.service.storageQuota.title", description: "settings.service.storageQuota.description" },
  { kind: "choice", section: "privacy", group: "settings.service.groups.keeping.title", title: "settings.service.retention.title", description: "settings.service.retention.description" },
  { kind: "choice", section: "privacy", group: "settings.service.groups.keeping.title", title: "settings.service.sensitive.title", description: "settings.service.sensitive.description", keywords: ["password", "key", "token", "delete"] },
  { kind: "boolean", section: "clipboard", group: "settings.service.groups.telling.title", title: "settings.service.notify.title", description: "settings.service.notify.description", keywords: ["notification"], capability: "copyNotifications" },
  { kind: "boolean", section: "clipboard", group: "settings.service.groups.telling.title", title: "settings.service.sound.title", description: "settings.service.sound.description" },
  { kind: "boolean", section: "device-sync", group: "settings.service.groups.network.title", title: "settings.service.syncEnabled.title", description: "settings.service.syncEnabled.description", keywords: ["pair", "devices"] },
  { kind: "boolean", section: "device-sync", group: "settings.service.groups.network.title", title: "settings.service.lan.title", description: "settings.service.lan.description", keywords: ["network", "discover"] },

  { kind: "custom", section: "clipboard", title: "capture.setup.enable.title", description: "capture.setup.enable.body", keywords: ["android", "shizuku", "permission", "clipboard"], platforms: ["android"] },
  { kind: "custom", section: "clipboard", title: "settings.service.exclusions.title", description: "settings.service.exclusions.androidLimitation", keywords: ["app", "application", "exclude", "source", "package", "privacy"], platforms: ["android"] },
  { kind: "custom", section: "clipboard", group: "capture.setup.always.title", title: "capture.setup.always.action", description: "capture.setup.always.body", platforms: ["android"] },
  { kind: "custom", section: "clipboard", group: "capture.setup.ladder.title", title: "capture.setup.ladder.armed", keywords: ["other apps", "shizuku", "permission"], platforms: ["android"] },
  { kind: "custom", section: "clipboard", title: "capture.toast.row.title", description: "capture.toast.row.body", keywords: ["android", "notice"], platforms: ["android"] },

  { kind: "custom", section: "device-sync", title: "devices.own.rename.label", description: "devices.own.rename.description", keywords: ["device name", "rename", "this device"] },
  { kind: "custom", section: "device-sync", title: "settings.sync.paired.title", description: "settings.sync.paired.description", keywords: ["pair", "devices", "encrypted"] },
  { kind: "action", section: "device-sync", title: "settings.sync.now.title", description: "settings.sync.now.description" },
  { kind: "custom", section: "cloud-sync", title: "settings.sync.cloud.connectionTitle", description: "settings.sync.cloud.description", keywords: ["cloud sync", "account", "internet"] },
  { kind: "custom", section: "cloud-sync", title: "settings.sync.cloud.endpoint.advancedTitle", description: "settings.sync.cloud.endpoint.advancedDescription", keywords: ["self-hosted", "server", "advanced"], disclosure: "cloud-server" },
  { kind: "custom", section: "cloud-sync", group: "settings.sync.cloud.endpoint.advancedTitle", title: "settings.sync.cloud.endpoint.title", description: "settings.sync.cloud.endpoint.description", keywords: ["server", "change", "restore"], disclosure: "cloud-server" },
  { kind: "custom", section: "cloud-sync", group: "settings.sync.cloud.endpoint.advancedTitle", title: "settings.sync.cloud.endpoint.url", keywords: ["host", "address", "server"], disclosure: "cloud-server" },
  { kind: "custom", section: "cloud-sync", group: "settings.sync.cloud.endpoint.advancedTitle", title: "settings.sync.cloud.endpoint.publishableKey", keywords: ["anon key", "server credential"], disclosure: "cloud-server" },

  { kind: "readonly", section: "storage", title: "settings.storage.stored.title" },
  { kind: "action", section: "storage", title: "settings.transfer.export.title", description: "settings.transfer.export.description" },
  { kind: "action", section: "storage", title: "settings.transfer.import.title", description: "settings.transfer.import.description" },
  { kind: "action", section: "storage", group: "settings.transfer.recoverySection", title: "settings.transfer.backup.title", description: "settings.transfer.backup.description" },
  { kind: "action", section: "storage", group: "settings.transfer.recoverySection", title: "settings.transfer.restore.title", description: "settings.transfer.restore.description" },
  { kind: "action", section: "storage", title: "settings.storage.clear.title", description: "settings.storage.clear.description" },

  { kind: "readonly", section: "diagnostics", group: "settings.diagnostics.running.title", title: "settings.diagnostics.running.history.title", description: "settings.diagnostics.running.history.description" },
  { kind: "readonly", section: "diagnostics", group: "settings.diagnostics.running.title", title: "settings.diagnostics.running.started.title", description: "settings.diagnostics.running.started.description" },
  { kind: "readonly", section: "diagnostics", group: "settings.diagnostics.dropped.title", title: "settings.diagnostics.dropped.tooLarge.title", description: "settings.diagnostics.dropped.tooLarge.description" },
  { kind: "readonly", section: "diagnostics", group: "settings.diagnostics.dropped.title", title: "settings.diagnostics.dropped.missed.title", description: "settings.diagnostics.dropped.missed.description" },
  { kind: "readonly", section: "diagnostics", group: "settings.diagnostics.dropped.title", title: "settings.diagnostics.dropped.swept.title", description: "settings.diagnostics.dropped.swept.description" },
  { kind: "readonly", section: "diagnostics", group: "settings.diagnostics.dropped.title", title: "settings.diagnostics.dropped.purged.title", description: "settings.diagnostics.dropped.purged.description" },
  { kind: "action", section: "diagnostics", title: "settings.diagnostics.report.title", keywords: ["copy", "export", "logs", "support"] },

  { kind: "custom", section: "runtime-events", title: "runtimeLog.title", keywords: ["logs", "events", "service", "activity"] },

  { kind: "readonly", section: "about", title: "settings.about.app.title", description: "settings.about.app.description" },
  { kind: "action", section: "about", title: "settings.about.updates.title", description: "settings.about.updates.description", keywords: ["update", "upgrade", "version"] },
  { kind: "readonly", section: "about", title: "settings.about.service.title" },
  { kind: "readonly", section: "about", title: "settings.about.capture.title", description: "settings.about.capture.description" },
  { kind: "readonly", section: "about", title: "settings.about.backend.title", description: "settings.about.backend.description" },
  { kind: "readonly", section: "about", title: "settings.about.protocol.title", description: "settings.about.protocol.description" },
  { kind: "readonly", section: "about", title: "settings.about.items.title", description: "settings.about.items.description" },
  { kind: "custom", section: "about", title: "settings.about.links.title" },
  { kind: "action", section: "about", group: "settings.about.links.title", title: "settings.about.links.repository" },
  { kind: "action", section: "about", group: "settings.about.links.title", title: "settings.about.links.releases" },
  { kind: "action", section: "about", title: "onboarding.settings.title", description: "onboarding.settings.description", keywords: ["setup", "onboarding", "welcome", "first run"] },
  { kind: "action", section: "about", title: "settings.about.reset.title", description: "settings.about.reset.description", keywords: ["defaults", "restore"] },
];
export const SETTINGS_FIELD_DEFINITIONS: readonly SettingsFieldDefinition[] = SEARCHABLE_FIELDS.map((field) => ({
  ...field,
  id: `${field.section}:${field.title}:${field.platforms?.join("-") ?? "all"}`,
  }));

export function settingDefinition(section: SettingsFieldDefinition["section"], title: string, platform?: SettingsPlatform): SettingsFieldDefinition {
  const definition = SETTINGS_FIELD_DEFINITIONS.find((field) => field.section === section && field.title === title && (platform ? field.platforms?.includes(platform) : !field.platforms));
  if (!definition) throw new Error(`Unknown settings field ${section}/${title}`);
  return definition;
}

const SECTION_DEFINITIONS: readonly PreferenceSectionDefinition[] = [
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
    value: "about",
    label: "About",
    description: "Versions, links and product information",
    icon: "help",
  },
];


export const SETTINGS_SECTIONS = SECTION_DEFINITIONS.map((section) => ({
  ...section,
  fields: SETTINGS_FIELD_DEFINITIONS.filter((field) => field.section === section.value || (section.value === "diagnostics" && field.section === "runtime-events")),
}));
