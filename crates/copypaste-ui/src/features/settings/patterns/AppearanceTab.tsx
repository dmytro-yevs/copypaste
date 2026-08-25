import { useId } from "react";

import { SettingsRow } from "@/components/shared";
import { Button, Switch } from "@/components/ui";
import { SettingsGroupSurface } from "@/features/settings/components/SettingsGroupSurface";
import { useTranslation } from "@/i18n";
import { changeAppearanceFrom } from "@/lib/themeTransition";
import {
  type ColorTheme,
  type ThemePref,
  usePrefs,
} from "@/store/prefs";
import styles from "./AppearanceTab.module.css";

const MODE_OPTIONS = [
  { value: "system", label: "settings.appearance.theme.system" },
  { value: "light", label: "settings.appearance.theme.light" },
  { value: "dark", label: "settings.appearance.theme.dark" },
] as const satisfies ReadonlyArray<{
  value: ThemePref;
  label: string;
}>;

const PRODUCT_THEMES = [
  {
    value: "midnight",
    label: "settings.appearance.colorTheme.midnight",
    description: "settings.appearance.colorTheme.midnightDescription",
  },
  {
    value: "aurora",
    label: "settings.appearance.colorTheme.aurora",
    description: "settings.appearance.colorTheme.auroraDescription",
  },
  {
    value: "ember",
    label: "settings.appearance.colorTheme.ember",
    description: "settings.appearance.colorTheme.emberDescription",
  },
  {
    value: "graphite",
    label: "settings.appearance.colorTheme.graphite",
    description: "settings.appearance.colorTheme.graphiteDescription",
  },
] as const satisfies ReadonlyArray<{
  value: ColorTheme;
  label: string;
  description: string;
}>;

export function AppearanceTab({
  ready,
  supportsTranslucency,
}: {
  ready: boolean;
  supportsTranslucency: boolean;
}) {
  const { t } = useTranslation();
  const theme = usePrefs((state) => state.theme);
  const colorTheme = usePrefs((state) => state.colorTheme);
  const translucency = usePrefs((state) => state.translucency);
  const set = usePrefs((state) => state.set);
  const themeDescriptionId = useId();
  const translucencyDescriptionId = useId();

  if (!ready) {
    return null;
  }

  return (
    <div className={styles.root}>
      <SettingsGroupSurface>
        <div className={styles.settingLabel}>
          <div>
            <strong id={themeDescriptionId}>{t("settings.appearance.theme.title")}</strong>
            <span>{t("settings.appearance.theme.override")}</span>
          </div>
          <div
            role="group"
            aria-label={t("settings.appearance.theme.title")}
            aria-describedby={themeDescriptionId}
            className={styles.modeControl}
          >
            {MODE_OPTIONS.map((option) => (
              <Button
                key={option.value}
                type="button"
                variant={theme === option.value ? "secondary" : "ghost"}
                size="sm"
                aria-label={t(option.label)}
                aria-pressed={theme === option.value}
                onClick={(event) => {
                  void changeAppearanceFrom(
                    event.currentTarget,
                    () => {
                      const current = usePrefs.getState();
                      return {
                        theme: option.value,
                        colorTheme: current.colorTheme,
                        translucency: current.translucency,
                      };
                    },
                    () => set("theme", option.value),
                  );
                }}
              >
                <span className={styles.modeLabel}>{t(option.label)}</span>
              </Button>
            ))}
          </div>
        </div>
      </SettingsGroupSurface>

      <div className={styles.themeTitle}>
        {t("settings.appearance.colorTheme.title")}
      </div>
      <div className={styles.themeGrid}>
        {PRODUCT_THEMES.map((option) => (
          <Button
            key={option.value}
            type="button"
            variant="ghost"
            size="md"
            aria-pressed={option.value === colorTheme}
            className={styles.themeCard}
            data-product-theme={option.value}
            onClick={(event) => {
              void changeAppearanceFrom(
                event.currentTarget,
                () => {
                  const current = usePrefs.getState();
                  return {
                    theme: current.theme,
                    colorTheme: option.value,
                    translucency: current.translucency,
                  };
                },
                () => set("colorTheme", option.value),
              );
            }}
          >
            <span className={styles.themePreview} aria-hidden="true">
              <span className={styles.themeRail}>
                <span className={styles.themeSwatches}>
                  <i />
                  <i />
                  <i />
                </span>
              </span>
              <span className={styles.themeCanvas}>
                <span className={styles.themeSwatches}>
                  <i />
                  <i />
                  <i />
                </span>
              </span>
            </span>
            <span className={styles.themeCopy}>
              <strong>{t(option.label)}</strong>
              <small>{t(option.description)}</small>
            </span>
          </Button>
        ))}
      </div>

      {supportsTranslucency ? (
        <SettingsGroupSurface>
          <SettingsRow
            title={t("settings.appearance.translucency.title")}
            descriptionId={translucencyDescriptionId}
            description={t("settings.appearance.translucency.description")}
          >
            <Switch
              aria-label={t("settings.appearance.translucency.title")}
              aria-describedby={translucencyDescriptionId}
              checked={translucency}
              onCheckedChange={(checked) => set("translucency", checked)}
            />
          </SettingsRow>
        </SettingsGroupSurface>
      ) : null}
    </div>
  );
}
