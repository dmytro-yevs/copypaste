// DisplayTab.tsx
// "Mask sensitive data" also lives in "History list" panel (kept, CopyPaste-h1n3).
// Appearance section (Theme, Accent, Translucency) restored — Slice 5 / task 5.3:
// wired to the same UIPrefs (prefs/setPrefs props, sourced from useUI in
// useSettingsState.ts) that already drove the pre-paint bootstrap and the
// removed Appearance section, so this is a rebuild of existing wiring, not new
// state/logic.
import { useEffect, useState } from "react";
import { SectionHeader } from "../../../components/SectionHeader";
import { SettingsRow } from "../../../components/SettingsRow";
import { Toggle } from "../../../components/Toggle";
import { Panel } from "../../../components/Panel";
import { SliderRow } from "../../../components/SliderRow";
import { InfoPopover } from "../components/InfoPopover";
import { THEME_VALUES, ACCENT_VALUES } from "../../../lib/theme/prefsSchema";
import type { UIPrefs } from "../../../store";

export type DisplayTabProps = {
  prefs: UIPrefs;
  setPrefs: (p: Partial<UIPrefs>) => void;
};

const THEME_LABEL: Record<(typeof THEME_VALUES)[number], string> = {
  system: "System",
  dark: "Dark",
  light: "Light",
};

// CopyPaste-8ebg.63: sliders showed a bare number ("3") instead of its unit.
function formatLines(v: number): string {
  return `${v} line${v === 1 ? "" : "s"}`;
}

export function DisplayTab({ prefs, setPrefs }: DisplayTabProps) {
  // CopyPaste-8ebg.63: "System" doesn't say what it actually resolves to.
  // Track the OS preference live so the hint updates if the user flips
  // System Settings → Appearance while this panel is open.
  const [systemIsDark, setSystemIsDark] = useState(
    () => typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: dark)").matches
  );
  useEffect(() => {
    if (typeof window === "undefined") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (e: MediaQueryListEvent) => setSystemIsDark(e.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  return (
    <div>
      {/* task 5.3: Theme / Accent / Translucency — bound directly to prefs.theme,
          prefs.accent, prefs.translucency via the prefs/setPrefs props (already
          the useUI-backed store values threaded down from useSettingsState.ts;
          no separate useUI() call needed here — same binding, same store). */}
      <SectionHeader label="Appearance" />
      <Panel>
        <SettingsRow
          title="Theme"
          info={<InfoPopover text="Dark or light chrome across the whole app. Applied immediately and persisted. 'System' follows the macOS Appearance setting." />}
        >
          <div className="ctl ctl--col">
            <div className="seg" role="group" aria-label="Theme">
              {THEME_VALUES.map((t) => (
                <button
                  key={t}
                  type="button"
                  className={prefs.theme === t ? "on" : undefined}
                  aria-pressed={prefs.theme === t}
                  onClick={() => setPrefs({ theme: t })}
                >
                  {THEME_LABEL[t]}
                </button>
              ))}
            </div>
            {/* CopyPaste-8ebg.63: "System" alone doesn't say what it resolves
                to — show the live-resolved light/dark value. */}
            {prefs.theme === "system" && (
              <span className="field-note field-note--dim">
                Currently resolves to {systemIsDark ? "Dark" : "Light"}.
              </span>
            )}
          </div>
        </SettingsRow>
        <SettingsRow
          title="Accent"
          info={<InfoPopover text="One accent color used across buttons, toggles, and highlights." />}
        >
          <div className="swatches" role="group" aria-label="Accent">
            {ACCENT_VALUES.map((a) => (
              // Each swatch is its own nested theme-scope so var(--accent)
              // resolves to THAT swatch's accent value, not the page's live
              // accent (matches GalleryView's swatch-preview pattern).
              <span key={a} className="theme-scope" data-accent={a}>
                <button
                  type="button"
                  className="swatch"
                  aria-label={a}
                  aria-pressed={prefs.accent === a}
                  style={{ background: "var(--accent)" }}
                  onClick={() => setPrefs({ accent: a })}
                />
              </span>
            ))}
          </div>
        </SettingsRow>
        <SettingsRow
          title="Translucency"
          info={<InfoPopover text="Frost chrome surfaces with a subtle blur. Turn off to render every surface solid." />}
        >
          <Toggle
            checked={prefs.translucency}
            onChange={(v) => setPrefs({ translucency: v })}
            aria-label="Translucency"
          />
        </SettingsRow>
      </Panel>

      <SectionHeader label="History list" />
      <Panel>
        {/* M4: split previewLines — main window has its own independent setting */}
        <SettingsRow
          title="Preview lines"
          info={<InfoPopover text="Number of text lines shown per clip in the main history window. Independent from the popup setting." />}
        >
          <SliderRow
            min={1}
            max={6}
            step={1}
            value={prefs.previewLinesApp}
            onChange={(v) => setPrefs({ previewLinesApp: v })}
            formatValue={formatLines}
          />
        </SettingsRow>
        {/* Image preview height controls the thumbnail bounding box in both
            the history list and the popup. */}
        <SettingsRow
          title="Image preview height"
          info={<InfoPopover text="Max height (px) of image thumbnails in the history list and the popup. The image scales to fit within 340 × height, aspect-preserving, never upscaled." />}
        >
          <SliderRow
            min={1}
            max={200}
            step={1}
            value={prefs.imageMaxHeight}
            onChange={(v) => setPrefs({ imageMaxHeight: v })}
            formatValue={(v) => `${v}px`}
          />
        </SettingsRow>
        {/* bdac.91: Group by device — persists the sort mode chosen in the History toolbar. */}
        <SettingsRow
          title="Group by device"
          info={<InfoPopover text="Group clipboard items by the device they came from, with your device shown first. You can also toggle this from the History toolbar when multiple devices are paired." />}
        >
          <Toggle
            checked={prefs.sortByDevice ?? false}
            onChange={(v) => setPrefs({ sortByDevice: v })}
          />
        </SettingsRow>
        {/* n9gp (PG-34): sensitive-reveal warning toggle — Android parity. */}
        <SettingsRow
          title="Warn before revealing sensitive items"
          info={<InfoPopover text="Show a confirmation overlay before revealing blurred sensitive content. Matches the Android warning sheet behaviour. Turn off if you find the extra step redundant." />}
        >
          <Toggle
            checked={prefs.showSensitiveWarnings ?? true}
            onChange={(v) => setPrefs({ showSensitiveWarnings: v })}
          />
        </SettingsRow>
        {/* Mask sensitive data — privacy control, relocated from Appearance (CopyPaste-h1n3). */}
        <SettingsRow
          title="Mask sensitive data"
          info={<InfoPopover text="Blur sensitive clipboard content (passwords, tokens, secrets) in history previews. Click a blurred item to reveal it." />}
        >
          <Toggle
            aria-label="Mask sensitive data"
            checked={prefs.maskSensitive ?? true}
            onChange={(v) => setPrefs({ maskSensitive: v })}
          />
        </SettingsRow>
      </Panel>

      <SectionHeader label="Popup appearance" hint="How the popup looks when triggered." />
      <Panel>
        {/* M4: popup gets its own independent preview-lines setting */}
        <SettingsRow
          title="Preview lines"
          info={<InfoPopover text="Number of text lines shown per clip in the Quick-Paste popup. Independent from the main window setting." />}
        >
          <SliderRow
            min={1}
            max={6}
            step={1}
            value={prefs.previewLinesPopup}
            onChange={(v) => setPrefs({ previewLinesPopup: v })}
            formatValue={formatLines}
          />
        </SettingsRow>
      </Panel>
    </div>
  );
}
