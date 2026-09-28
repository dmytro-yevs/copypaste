import type { SettingsField, SettingsGroupSchema, SettingsDisclosureId } from "./settingsFieldSchema";
import type { PreferenceSection } from "./preferenceSections";
import { SETTINGS_GROUP_DEFINITIONS, settingDefinition } from "./settingsSchemaCatalog";

interface ProjectionOptions {
  readonly revealKeys?: Partial<Record<SettingsDisclosureId, string | undefined>>;
  readonly groupDescriptions?: Readonly<Record<string, string>>;
}

/** Bind live values to canonical group/field layout. Section owners cannot
 * create a second label, group membership or order through this API. */
export function settingsGroups(
  section: PreferenceSection,
  bindings: readonly SettingsField[],
  translate: (key: string) => string,
  options: ProjectionOptions = {},
): readonly SettingsGroupSchema[] {
  const byId = new Map<string, SettingsField>();
  for (const binding of bindings) {
    if (byId.has(binding.definition.id)) throw new Error(`Duplicate settings binding ${binding.definition.id}`);
    byId.set(binding.definition.id, binding);
  }
  const used = new Set<string>();
  const groups = SETTINGS_GROUP_DEFINITIONS.filter((group) => group.section === section).map((group) => {
    const fields = group.fields.flatMap((title) => {
      const owner = section === "diagnostics" && title === "runtimeLog.title" ? "runtime-events" : section;
      const definition = settingDefinition(owner, title);
      const binding = byId.get(definition.id);
      if (!binding) return [];
      used.add(definition.id);
      return [{ ...binding, definition } as SettingsField];
    });
    return {
      id: group.id,
      title: group.title ? translate(group.title) : undefined,
      description: options.groupDescriptions?.[group.id] ?? (group.description ? translate(group.description) : undefined),
      surface: group.surface,
      disclosure: group.disclosure,
      revealKey: group.disclosure ? options.revealKeys?.[group.disclosure] : undefined,
      fields,
    } satisfies SettingsGroupSchema;
  }).filter((group) => group.fields.some((field) => field.visible !== false));
  for (const id of byId.keys()) if (!used.has(id)) throw new Error(`Unplaced settings binding ${id}`);
  return groups;
}
