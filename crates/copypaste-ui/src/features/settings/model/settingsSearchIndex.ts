import type {
  SettingsCapabilities,
} from "./settingsNavigation";
import type { PreferenceSection } from "./preferenceSections";

export type SettingsSearchTab = PreferenceSection | "runtime-events";

export interface SettingsSearchItem {
  tab: SettingsSearchTab;
  section?: string;
  title: string;
  description?: string;
  keywords?: readonly string[];
  platforms?: readonly ("desktop" | "android" | "windows")[];
  capability?: Exclude<keyof SettingsCapabilities, "platform">;
  disclosure?: "clipboard-advanced" | "cloud-server";
}

export interface SettingsDisclosureReveal {
  readonly owner: NonNullable<SettingsSearchItem["disclosure"]>;
  readonly key: string;
}

export function disclosureRevealKey(
  reveal: SettingsDisclosureReveal | undefined,
  owner: SettingsDisclosureReveal["owner"],
): string | undefined {
  return reveal?.owner === owner ? reveal.key : undefined;
}

/** Every settings row is listed here so search does not depend on hidden tabs
 * being mounted. Translation keys keep the index correct when copy changes. */
import { SETTINGS_FIELD_DEFINITIONS, SETTINGS_GROUP_DEFINITIONS, SETTINGS_SECTIONS } from "./settingsSchemaCatalog";

const searchDefinitions = [
  ...SETTINGS_SECTIONS.flatMap((section) => section.fields),
  ...SETTINGS_FIELD_DEFINITIONS.filter((field) => field.kind === "group"),
];

export const SETTINGS_SEARCH_ITEMS: readonly SettingsSearchItem[] = searchDefinitions.map((field) => ({
  tab: field.section,
  section: SETTINGS_GROUP_DEFINITIONS.find((group) =>
    group.section === (field.section === "runtime-events" ? "diagnostics" : field.section) &&
    group.fields.includes(field.title),
  )?.title,
  title: field.title,
  description: field.description,
  keywords: field.keywords,
  platforms: field.platforms,
  capability: field.capability,
  disclosure: field.disclosure,
}));
