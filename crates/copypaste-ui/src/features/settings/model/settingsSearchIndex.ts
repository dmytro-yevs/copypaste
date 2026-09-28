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
import { SETTINGS_FIELD_DEFINITIONS } from "./settingsSchemaCatalog";

export const SETTINGS_SEARCH_ITEMS: readonly SettingsSearchItem[] = SETTINGS_FIELD_DEFINITIONS.map((field) => ({
  tab: field.section,
  section: field.group,
  title: field.title,
  description: field.description,
  keywords: field.keywords,
  platforms: field.platforms,
  capability: field.capability,
  disclosure: field.disclosure,
}));
