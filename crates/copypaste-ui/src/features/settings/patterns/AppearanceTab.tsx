import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import type { SettingsField } from "@/features/settings/model/settingsFieldSchema";
import { settingsGroups } from "@/features/settings/model/settingsProjection";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import { useTranslation } from "@/i18n";
import { changeAppearanceFrom } from "@/lib/themeTransition";
import { type ColorTheme, type ThemePref, usePrefs } from "@/store/prefs";
import styles from "./AppearanceTab.module.css";

const MODE_OPTIONS = [
  { value: "system", label: "settings.appearance.theme.system" },
  { value: "light", label: "settings.appearance.theme.light" },
  { value: "dark", label: "settings.appearance.theme.dark" },
] as const satisfies ReadonlyArray<{ value: ThemePref; label: string }>;
const PRODUCT_THEMES = [
  { value: "midnight", label: "settings.appearance.colorTheme.midnight", description: "settings.appearance.colorTheme.midnightDescription" },
  { value: "aurora", label: "settings.appearance.colorTheme.aurora", description: "settings.appearance.colorTheme.auroraDescription" },
  { value: "ember", label: "settings.appearance.colorTheme.ember", description: "settings.appearance.colorTheme.emberDescription" },
  { value: "graphite", label: "settings.appearance.colorTheme.graphite", description: "settings.appearance.colorTheme.graphiteDescription" },
] as const satisfies ReadonlyArray<{ value: ColorTheme; label: string; description: string }>;

export function AppearanceTab({ ready, supportsTranslucency }: { ready: boolean; supportsTranslucency: boolean }) {
  const { t } = useTranslation();
  const theme = usePrefs((state) => state.theme);
  const colorTheme = usePrefs((state) => state.colorTheme);
  const translucency = usePrefs((state) => state.translucency);
  const set = usePrefs((state) => state.set);
  if (!ready) return null;
  const fields: SettingsField[] = [
    { kind: "choice", definition: settingDefinition("appearance", "settings.appearance.theme.title"), value: theme, presentation: "segmented", controlClassName: styles.modeControl,
      options: MODE_OPTIONS.map((option) => ({ value: option.value, label: t(option.label) })),
      renderOption: (option) => <span className={styles.modeLabel}>{option.label}</span>,
      onChange: (value, source) => { if (source) void changeAppearanceFrom(source, () => { const current = usePrefs.getState(); return { theme: value as ThemePref, colorTheme: current.colorTheme, translucency: current.translucency }; }, () => set("theme", value as ThemePref)); },
    },
    { kind: "choice", definition: settingDefinition("appearance", "settings.appearance.colorTheme.title"), value: colorTheme, presentation: "cards", controlClassName: styles.themeGrid,
      optionClassName: styles.themeCard, titleClassName: styles.themeTitle,
      options: PRODUCT_THEMES.map((option) => ({ value: option.value, label: t(option.label), description: t(option.description) })),
      renderOption: (option) => <><span className={styles.themePreview} aria-hidden="true"><span className={styles.themeRail}><span className={styles.themeSwatches}><i /><i /><i /></span></span><span className={styles.themeCanvas}><span className={styles.themeSwatches}><i /><i /><i /></span></span></span><span className={styles.themeCopy}><strong>{option.label}</strong><small>{option.description}</small></span></>,
      onChange: (value, source) => { if (source) void changeAppearanceFrom(source, () => { const current = usePrefs.getState(); return { theme: current.theme, colorTheme: value as ColorTheme, translucency: current.translucency }; }, () => set("colorTheme", value as ColorTheme)); },
    },
    { kind: "boolean", definition: settingDefinition("appearance", "settings.appearance.translucency.title"), value: translucency, visible: supportsTranslucency, onChange: (value) => set("translucency", value) },
  ];
  return <div className={styles.root}><SettingsSchemaRenderer groups={settingsGroups("appearance", fields, (key) => t(key as never))} /></div>;
}
