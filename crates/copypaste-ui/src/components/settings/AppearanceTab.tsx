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

import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Label } from "@/components/ui/label";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import { resolveTheme } from "@/lib/theme";
import { ACCENTS, type Accent, type ThemePref, usePrefs } from "@/store/prefs";
import { Row } from "@/components/settings/Row";

const THEME_OPTIONS = [
  { value: "system", label: "settings.appearance.theme.system" },
  { value: "dark", label: "settings.appearance.theme.dark" },
  { value: "light", label: "settings.appearance.theme.light" },
] as const satisfies ReadonlyArray<{ value: ThemePref; label: string }>;

const ACCENT_LABEL = {
  indigo: "settings.appearance.accent.indigo",
  blue: "settings.appearance.accent.blue",
  teal: "settings.appearance.accent.teal",
  green: "settings.appearance.accent.green",
  amber: "settings.appearance.accent.amber",
  rose: "settings.appearance.accent.rose",
} as const satisfies Record<Accent, string>;

/** Subscribe to the OS appearance so the "resolves to" hint is live. */
function useSystemTheme(): "dark" | "light" {
  return useSyncExternalStore(
    (notify) => {
      if (!window.matchMedia) return () => {};
      const media = window.matchMedia("(prefers-color-scheme: dark)");
      media.addEventListener("change", notify);
      return () => media.removeEventListener("change", notify);
    },
    () => resolveTheme("system"),
    () => "dark" as const,
  );
}

export function AppearanceTab() {
  const { t } = useTranslation();
  const theme = usePrefs((s) => s.theme);
  const accent = usePrefs((s) => s.accent);
  const translucency = usePrefs((s) => s.translucency);
  const set = usePrefs((s) => s.set);
  const system = useSystemTheme();

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
                data-accent={option}
                onClick={() => set("accent", option)}
                className={cn(
                  "flex size-7 items-center justify-center rounded-full bg-brand text-on-brand outline-none transition-transform focus-visible:ring-[3px] focus-visible:ring-ring",
                  selected ? "scale-110" : "hover:scale-105",
                )}
              >
                {selected && <Check size={14} aria-hidden="true" />}
              </button>
            );
          })}
        </div>
      </Row>

      <Row
        title={t("settings.appearance.translucency.title")}
        description={t("settings.appearance.translucency.description")}
      >
        <div className="flex items-center gap-s-2">
          <Switch
            id="translucency"
            checked={translucency}
            onCheckedChange={(value) => set("translucency", value)}
          />
          <Label htmlFor="translucency">
            {t(translucency ? "common.on" : "common.off")}
          </Label>
        </div>
      </Row>
    </div>
  );
}
