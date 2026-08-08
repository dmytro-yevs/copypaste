/**
 * Two details are bug fixes rather than decoration:
 *
 *  - "System" says what it currently resolves to, live (CopyPaste-8ebg.63): a
 *    segmented control that reads System / Dark / Light tells the user nothing
 *    about what they are looking at.
 *  - The accent swatches are a `role="group"` of real buttons with
 *    `aria-pressed` (A11Y-8), not a row of coloured divs. Each carries a name,
 *    because colour is the only other thing distinguishing them (A11Y-9).
 */
import { useSyncExternalStore } from "react";
import { Check } from "lucide-react";

import { Slider } from "@/components/ui/slider";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useTranslation } from "@/i18n";
import { APPEARANCE_SERIALIZATION } from "@/lib/appearancePrefs";
import { resolveTheme } from "@/lib/theme";
import { isAndroidPlatform } from "@/lib/platform";
import {
  ACCENTS,
  type Accent,
  type ThemePref,
  usePrefs,
} from "@/store/prefs";
import { Row } from "@/components/settings/Row";

const THEME_OPTIONS = [
  { value: "system", label: "settings.appearance.theme.system" },
  { value: "dark", label: "settings.appearance.theme.dark" },
  { value: "light", label: "settings.appearance.theme.light" },
] as const satisfies ReadonlyArray<{ value: ThemePref; label: string }>;

const ACCENT_LABEL = {
  system: "settings.appearance.accent.system",
  indigo: "settings.appearance.accent.indigo",
  blue: "settings.appearance.accent.blue",
  teal: "settings.appearance.accent.teal",
  green: "settings.appearance.accent.green",
  amber: "settings.appearance.accent.amber",
  rose: "settings.appearance.accent.rose",
} as const satisfies Record<Accent, string>;

const ACCENT_CONTROL_CLASS = "flex h-7 shrink-0 items-center justify-center rounded-full outline-none transition-transform hover:scale-105 focus-visible:ring-[3px] focus-visible:ring-ring";

/** Subscribe to the OS appearance so the "resolves to" hint is live. */
function useSystemTheme(): "dark" | "light" {
  return useSyncExternalStore(
    (notify) => {
      if (!window.matchMedia) return () => {};
      // The contract's query, not a second copy of the string: `resolveTheme`
      // below reads the same one, and a drift between them would subscribe to
      // one condition and report another.
      const media = window.matchMedia(APPEARANCE_SERIALIZATION.systemThemeQuery);
      media.addEventListener("change", notify);
      return () => media.removeEventListener("change", notify);
    },
    () => resolveTheme("system"),
    () => APPEARANCE_SERIALIZATION.systemThemeFallback,
  );
}

export function AppearanceTab() {
  const { t } = useTranslation();
  const theme = usePrefs((s) => s.theme);
  const accent = usePrefs((s) => s.accent);
  const translucency = usePrefs((s) => s.translucency);
  const set = usePrefs((s) => s.set);
  const system = useSystemTheme();
  const supportsWindowTranslucency = !isAndroidPlatform();

  return (
    <div className="flex flex-col">
      <Row
        title={t("settings.appearance.theme.title")}
        description={
          theme === "system"
            ? t("settings.appearance.theme.resolves", {
                theme: t(
                  system === "dark"
                    ? "settings.appearance.theme.dark"
                    : "settings.appearance.theme.light",
                ),
              })
            : t("settings.appearance.theme.override")
        }
      >
        <ToggleGroup
          type="single"
          value={theme}
          equalWidth
          aria-label={t("settings.appearance.theme.title")}
          onValueChange={(value) => value && set("theme", value as ThemePref)}
        >
          {THEME_OPTIONS.map((option) => (
            <ToggleGroupItem key={option.value} value={option.value}>
              {t(option.label)}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </Row>

      <Row
        title={t("settings.appearance.accent.title")}
        description={t("settings.appearance.accent.description")}
      >
        <div
          role="group"
          aria-label={t("settings.appearance.accent.title")}
          className="flex flex-wrap gap-s-2"
        >
          {ACCENTS.map((option) => {
            const selected = option === accent;
            return (
              <button
                key={option}
                type="button"
                aria-pressed={selected}
                aria-label={t(ACCENT_LABEL[option])}
                title={t(ACCENT_LABEL[option])}
                data-swatch={option}
                onClick={() => set("accent", option)}
                style={{
                  backgroundColor:
                    option === "system"
                      ? "var(--system-accent-preview, var(--swatch-indigo))"
                      : `var(--swatch-${option})`,
                  color:
                    option === "system"
                      ? "var(--system-on-accent, var(--on-swatch-indigo))"
                      : `var(--on-swatch-${option})`,
                }}
                className={`${ACCENT_CONTROL_CLASS} ${
                  option === "system" ? "gap-1 px-2 text-xs font-medium" : "size-7"
                }`}
              >
                {option === "system" && <span>System</span>}
                {selected && <Check size={14} aria-hidden="true" />}
              </button>
            );
          })}
        </div>
      </Row>

      {supportsWindowTranslucency && (
        <Row
          title={t("settings.appearance.translucency.title")}
          description={t("settings.appearance.translucency.description")}
        >
          <div className="flex w-[220px] items-center gap-s-3 max-sm:w-full">
            <Slider
              aria-label={t("settings.appearance.translucency.title")}
              value={[translucency]}
              min={0}
              max={100}
              step={5}
              onValueChange={([value]) => value !== undefined && set("translucency", value)}
            />
            <output className="w-10 shrink-0 text-right text-xs tabular-nums text-muted-foreground">
              {translucency}%
            </output>
          </div>
        </Row>
      )}
    </div>
  );
}
