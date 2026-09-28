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
  { kind: "boolean", section: "shortcuts", title: "settings.startup.openAtLogin.title", description: "settings.startup.openAtLogin.description", keywords: ["autostart", "login", "sign in", "boot", "launch", "start with windows", "login items"], capability: "startup" },

  { kind: "action", section: "clipboard", title: "capture.title", description: "capture.loading.body", keywords: ["background", "clipboard", "recording", "paused", "android", "shizuku", "permission", "other apps", "notice", "always on"] },
  { kind: "boolean", section: "privacy", title: "settings.service.privateMode.title", description: "settings.service.privateMode.description" },
  { kind: "choice", section: "clipboard", title: "settings.service.poll.title", description: "settings.service.poll.description", keywords: ["polling", "interval", "frequency"], disclosure: "clipboard-advanced" },
  { kind: "choice", section: "clipboard", title: "settings.service.dedup.title", description: "settings.service.dedup.description" },
  { kind: "choice", section: "clipboard", title: "settings.service.maxText.title", description: "settings.service.maxText.description", disclosure: "clipboard-advanced" },
  { kind: "choice", section: "clipboard", title: "settings.service.maxImage.title", description: "settings.service.maxImage.description", disclosure: "clipboard-advanced" },
  { kind: "choice", section: "clipboard", title: "settings.service.maxFile.title", description: "settings.service.maxFile.description", disclosure: "clipboard-advanced" },
  { kind: "choice", section: "clipboard", title: "settings.service.maxDecodedImage.title", description: "settings.service.maxDecodedImage.description", disclosure: "clipboard-advanced" },
  { kind: "custom", section: "clipboard", title: "settings.service.exclusions.title", description: "settings.service.exclusions.description", keywords: ["app", "application", "exclude", "source", "bundle", "package", "privacy", "program", "exe", "android"] },
  { kind: "choice", section: "privacy", title: "settings.service.historyLimit.title", description: "settings.service.historyLimit.description" },
  { kind: "choice", section: "privacy", title: "settings.service.storageQuota.title", description: "settings.service.storageQuota.description" },
  { kind: "choice", section: "privacy", title: "settings.service.retention.title", description: "settings.service.retention.description" },
  { kind: "choice", section: "privacy", title: "settings.service.sensitive.title", description: "settings.service.sensitive.description", keywords: ["password", "key", "token", "delete"] },
  { kind: "boolean", section: "clipboard", title: "settings.service.notify.title", description: "settings.service.notify.description", keywords: ["notification"], capability: "copyNotifications" },
  { kind: "boolean", section: "clipboard", title: "settings.service.sound.title", description: "settings.service.sound.description" },
  { kind: "boolean", section: "device-sync", title: "settings.service.syncEnabled.title", description: "settings.service.syncEnabled.description", keywords: ["pair", "devices"] },
  { kind: "boolean", section: "device-sync", title: "settings.service.lan.title", description: "settings.service.lan.description", keywords: ["network", "discover"] },


  { kind: "custom", section: "device-sync", title: "devices.own.rename.label", description: "devices.own.rename.description", keywords: ["device name", "rename", "this device"] },
  { kind: "custom", section: "device-sync", title: "settings.sync.paired.title", description: "settings.sync.paired.description", keywords: ["pair", "devices", "encrypted"] },
  { kind: "action", section: "device-sync", title: "settings.sync.now.title", description: "settings.sync.now.description" },
  { kind: "status", section: "cloud-sync", title: "settings.sync.cloud.connectionTitle", description: "settings.sync.cloud.description", keywords: ["cloud sync", "account", "internet"] },
  { kind: "action", section: "cloud-sync", title: "settings.sync.cloud.setupTitle", description: "settings.sync.cloud.setupDescription" },
  { kind: "custom", section: "cloud-sync", title: "settings.sync.cloud.accountTitle" },
  { kind: "group", section: "cloud-sync", title: "settings.sync.cloud.endpoint.advancedTitle", description: "settings.sync.cloud.endpoint.advancedDescription", keywords: ["self-hosted", "server", "advanced"], disclosure: "cloud-server" },
  { kind: "action", section: "cloud-sync", title: "settings.sync.cloud.endpoint.title", description: "settings.sync.cloud.endpoint.description", keywords: ["server", "change", "restore"], disclosure: "cloud-server" },
  { kind: "custom", section: "cloud-sync", title: "settings.sync.cloud.endpoint.url", keywords: ["host", "address", "server"], disclosure: "cloud-server" },
  { kind: "custom", section: "cloud-sync", title: "settings.sync.cloud.endpoint.publishableKey", keywords: ["anon key", "server credential"], disclosure: "cloud-server" },

  { kind: "readonly", section: "storage", title: "settings.storage.stored.title" },
  { kind: "action", section: "storage", title: "settings.transfer.export.title", description: "settings.transfer.export.description" },
  { kind: "action", section: "storage", title: "settings.transfer.import.title", description: "settings.transfer.import.description" },
  { kind: "action", section: "storage", title: "settings.transfer.backup.title", description: "settings.transfer.backup.description" },
  { kind: "action", section: "storage", title: "settings.transfer.restore.title", description: "settings.transfer.restore.description" },
  { kind: "action", section: "storage", title: "settings.storage.clear.title", description: "settings.storage.clear.description" },

  { kind: "status", section: "diagnostics", title: "settings.diagnostics.running.history.title", description: "settings.diagnostics.running.history.description" },
  { kind: "readonly", section: "diagnostics", title: "settings.diagnostics.running.started.title", description: "settings.diagnostics.running.started.description" },
  { kind: "dynamic", section: "diagnostics", title: "settings.diagnostics.dropped.tooLarge.title", description: "settings.diagnostics.dropped.tooLarge.description" },
  { kind: "readonly", section: "diagnostics", title: "settings.diagnostics.dropped.missed.title", description: "settings.diagnostics.dropped.missed.description" },
  { kind: "readonly", section: "diagnostics", title: "settings.diagnostics.dropped.swept.title", description: "settings.diagnostics.dropped.swept.description" },
  { kind: "readonly", section: "diagnostics", title: "settings.diagnostics.dropped.purged.title", description: "settings.diagnostics.dropped.purged.description" },
  { kind: "action", section: "diagnostics", title: "settings.diagnostics.report.title", keywords: ["copy", "export", "logs", "support"] },

  { kind: "dynamic", section: "runtime-events", title: "runtimeLog.title", keywords: ["logs", "events", "service", "activity"] },

  { kind: "readonly", section: "about", title: "settings.about.app.title", description: "settings.about.app.description" },
  { kind: "dynamic", section: "about", title: "settings.about.updates.title", description: "settings.about.updates.description", keywords: ["update", "upgrade", "version"] },
  { kind: "status", section: "about", title: "settings.about.service.title" },
  { kind: "status", section: "about", title: "settings.about.capture.title", description: "settings.about.capture.description" },
  { kind: "readonly", section: "about", title: "settings.about.backend.title", description: "settings.about.backend.description" },
  { kind: "readonly", section: "about", title: "settings.about.protocol.title", description: "settings.about.protocol.description" },
  { kind: "readonly", section: "about", title: "settings.about.items.title", description: "settings.about.items.description" },
  { kind: "group", section: "about", title: "settings.about.links.title" },
  { kind: "action", section: "about", title: "settings.about.links.repository" },
  { kind: "action", section: "about", title: "settings.about.links.releases" },
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



export interface SettingsGroupDefinition {
  readonly section: PreferenceSectionDefinition["value"];
  readonly id: string;
  readonly title?: string;
  readonly description?: string;
  readonly disclosure?: SettingsFieldDefinition["disclosure"];
  readonly surface?: boolean;
  readonly fields: readonly string[];
}

/** Field order and group ownership are defined once here. Controllers bind
 * live values, callbacks and rare workflow slots without recreating layouts. */
export const SETTINGS_GROUP_DEFINITIONS: readonly SettingsGroupDefinition[] = [
  { section: "appearance", id: "mode", fields: ["settings.appearance.theme.title"] },
  { section: "appearance", id: "color-theme", surface: false, fields: ["settings.appearance.colorTheme.title"] },
  { section: "appearance", id: "translucency", fields: ["settings.appearance.translucency.title"] },
  { section: "clipboard", id: "capture-status", fields: ["capture.title"] },
  { section: "clipboard", id: "capture", title: "settings.service.groups.capture.title", description: "settings.service.groups.capture.description", fields: ["settings.service.dedup.title", "settings.service.exclusions.title"] },
  { section: "clipboard", id: "clipboard-advanced", title: "settings.service.advanced.title", description: "settings.service.advanced.description", disclosure: "clipboard-advanced", fields: ["settings.service.poll.title", "settings.service.maxText.title", "settings.service.maxImage.title", "settings.service.maxFile.title", "settings.service.maxDecodedImage.title"] },
  { section: "clipboard", id: "notifications", title: "settings.service.groups.telling.title", fields: ["settings.service.notify.title", "settings.service.sound.title"] },
  { section: "clipboard", id: "history-list", title: "History list", fields: ["settings.list.groupByDevice.title", "settings.list.historyDisplayLimit.title"] },
  { section: "privacy", id: "private-mode", title: "Private mode", fields: ["settings.service.privateMode.title"] },
  { section: "privacy", id: "retention", title: "settings.service.groups.keeping.title", fields: ["settings.service.historyLimit.title", "settings.service.storageQuota.title", "settings.service.retention.title", "settings.service.sensitive.title"] },
  { section: "privacy", id: "reveal-protection", title: "Reveal protection", fields: ["settings.list.warnBeforeReveal.title", "settings.list.allowScreenshots.title"] },
  { section: "shortcuts", id: "shortcut", fields: ["settings.shortcut.title"] },
  { section: "shortcuts", id: "startup", title: "settings.startup.title", fields: ["settings.startup.openAtLogin.title"] },
  { section: "device-sync", id: "devices", title: "Devices", fields: ["devices.own.rename.label", "settings.sync.paired.title", "settings.sync.now.title"] },
  { section: "device-sync", id: "network", title: "settings.service.groups.network.title", fields: ["settings.service.syncEnabled.title", "settings.service.lan.title"] },
  { section: "cloud-sync", id: "cloud-connection", title: "settings.sync.cloud.sectionTitle", description: "settings.sync.cloud.sectionDescription", fields: ["settings.sync.cloud.connectionTitle", "settings.sync.cloud.setupTitle", "settings.sync.cloud.accountTitle"] },
  { section: "cloud-sync", id: "cloud-server", title: "settings.sync.cloud.endpoint.advancedTitle", description: "settings.sync.cloud.endpoint.advancedDescription", disclosure: "cloud-server", fields: ["settings.sync.cloud.endpoint.title", "settings.sync.cloud.endpoint.url", "settings.sync.cloud.endpoint.publishableKey"] },
  { section: "storage", id: "history", title: "settings.storage.historySection", fields: ["settings.storage.stored.title"] },
  { section: "storage", id: "transfer", title: "settings.transfer.transferSection", fields: ["settings.transfer.export.title", "settings.transfer.import.title"] },
  { section: "storage", id: "recovery", title: "settings.transfer.recoverySection", fields: ["settings.transfer.backup.title", "settings.transfer.restore.title"] },
  { section: "storage", id: "danger", title: "settings.storage.dangerSection", fields: ["settings.storage.clear.title"] },
  { section: "diagnostics", id: "running", title: "settings.diagnostics.running.title", fields: ["settings.diagnostics.running.history.title", "settings.diagnostics.running.started.title"] },
  { section: "diagnostics", id: "dropped", title: "settings.diagnostics.dropped.title", description: "settings.diagnostics.dropped.description", fields: ["settings.diagnostics.dropped.tooLarge.title", "settings.diagnostics.dropped.missed.title", "settings.diagnostics.dropped.swept.title", "settings.diagnostics.dropped.purged.title"] },
  { section: "diagnostics", id: "support", title: "Support", fields: ["settings.diagnostics.report.title"] },
  { section: "diagnostics", id: "runtime-events", title: "runtimeLog.title", description: "runtimeLog.description", fields: ["runtimeLog.title"] },
  { section: "about", id: "identity", surface: false, fields: ["settings.about.app.title"] },
  { section: "about", id: "updates", fields: ["settings.about.updates.title"] },
  { section: "about", id: "runtime", title: "settings.about.runtime.title", fields: ["settings.about.service.title", "settings.about.capture.title", "settings.about.backend.title", "settings.about.protocol.title", "settings.about.items.title"] },
  { section: "about", id: "links", title: "settings.about.links.title", fields: ["settings.about.links.repository", "settings.about.links.releases"] },
  { section: "about", id: "welcome", fields: ["onboarding.settings.title"] },
  { section: "about", id: "reset", fields: ["settings.about.reset.title"] },
];

export const SETTINGS_SECTIONS = SECTION_DEFINITIONS.map((section) => {
  const groups = SETTINGS_GROUP_DEFINITIONS.filter((group) => group.section === section.value);
  return {
    ...section,
    groups,
    fields: groups.flatMap((group) => group.fields.map((title) => settingDefinition(
      section.value === "diagnostics" && title === "runtimeLog.title" ? "runtime-events" : section.value,
      title,
    ))),
  };
});
