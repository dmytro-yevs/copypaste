/**
 * Settings › Appearance — theme, accent, translucency.
 *
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
import { cn } from "@/lib/cn";
import { resolveTheme } from "@/lib/theme";
import { ACCENTS, type Accent, type ThemePref, usePrefs } from "@/store/prefs";
import { Row } from "@/components/settings/Row";

const THEME_OPTIONS: ReadonlyArray<{ value: ThemePref; label: string }> = [
  { value: "system", label: "System" },
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
];

const ACCENT_LABEL: Record<Accent, string> = {
  indigo: "Indigo",
  blue: "Blue",
  teal: "Teal",
  green: "Green",
  amber: "Amber",
  rose: "Rose",
};

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
  const theme = usePrefs((s) => s.theme);
  const accent = usePrefs((s) => s.accent);
  const translucency = usePrefs((s) => s.translucency);
  const set = usePrefs((s) => s.set);
  const system = useSystemTheme();

  return (
    <div className="flex flex-col">
      <Row
        title="Theme"
        description={
          theme === "system"
            ? `Currently resolves to ${system === "dark" ? "Dark" : "Light"}.`
            : "Overrides the system appearance for CopyPaste only."
        }
      >
        <ToggleGroup
          type="single"
          value={theme}
          aria-label="Theme"
          onValueChange={(value) => value && set("theme", value as ThemePref)}
        >
          {THEME_OPTIONS.map((option) => (
            <ToggleGroupItem key={option.value} value={option.value}>
              {option.label}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </Row>

      <Row
        title="Accent"
        description="Tints selection, focus rings and primary buttons."
      >
        <div role="group" aria-label="Accent" className="flex flex-wrap gap-s-2">
          {ACCENTS.map((option) => {
            const selected = option === accent;
            return (
              <button
                key={option}
                type="button"
                aria-pressed={selected}
                aria-label={ACCENT_LABEL[option]}
                title={ACCENT_LABEL[option]}
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
        title="Translucency"
        description="Frost the window chrome over what is behind it. Turned off automatically when the system asks for reduced transparency."
      >
        <div className="flex items-center gap-s-2">
          <Switch
            id="translucency"
            checked={translucency}
            onCheckedChange={(value) => set("translucency", value)}
          />
          <Label htmlFor="translucency">{translucency ? "On" : "Off"}</Label>
        </div>
      </Row>
    </div>
  );
}
