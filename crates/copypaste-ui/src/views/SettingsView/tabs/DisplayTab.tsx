// DisplayTab.tsx — Phase 3 redesign (CopyPaste-2hfj.4)
// Appearance tab: Theme segmented control + Accent swatch picker + optional toggles.
// Source of truth: docs/design/STYLEGUIDE.md §9.2 (segmented), §3.5 (accents), §2 (two-axis).
//
// Removed from this tab (§2 STYLEGUIDE):
//   - Old appearance pickers (deleted in Phase 1–2)
//   - Row density control (removed in Phase 2)
//   - Reduce motion toggle (removed in Phase 2)
//   - Color theme "System" option (§2: dark|light only)
import { SectionHeader } from "../../../components/SectionHeader";
import { SettingsRow } from "../../../components/SettingsRow";
import { Toggle } from "../../../components/Toggle";
import { Panel } from "../../../components/Panel";
import { SliderRow } from "../../../components/SliderRow";
import { InfoPopover } from "../components/InfoPopover";
import type { AccentId, UIPrefs } from "../../../store";

// §3.5 accent swatch preview colors (dark-theme base values).
// Hardcoded hex is intentional here — these are the static preview swatches,
// not interactive color tokens. The live --accent CSS variable takes over for
// all other interactive/brand uses once the accent is selected.
const ACCENT_SWATCHES: ReadonlyArray<{ id: AccentId; label: string; color: string }> = [
  { id: "indigo", label: "Indigo", color: "#6E5BFF" },
  { id: "blue",   label: "Blue",   color: "#3B82F6" },
  { id: "teal",   label: "Teal",   color: "#13B8A6" },
  { id: "green",  label: "Green",  color: "#46C56A" },
  { id: "amber",  label: "Amber",  color: "#F5A524" },
  { id: "rose",   label: "Rose",   color: "#F43F7E" },
];

export type DisplayTabProps = {
  prefs: UIPrefs;
  setPrefs: (p: Partial<UIPrefs>) => void;
};

export function DisplayTab({ prefs, setPrefs }: DisplayTabProps) {
  // §2: two axes — theme defaults to dark, accent defaults to indigo.
  const activeTheme = prefs.theme ?? "dark";
  const activeAccent = prefs.accent ?? "indigo";

  return (
    <div className="space-y-2">
      {/* ── Appearance ─────────────────────────────────────────────────────── */}
      {/* §2 STYLEGUIDE: The only appearance choices are theme (dark/light) and accent (6 hues). */}
      <SectionHeader label="Appearance" />
      <Panel>
        {/* Theme — §9.2 segmented control: Light / Dark.
            Container: --card bg + --border + --r-ctl radius.
            Active segment: --raised bg + --text + font-medium + --r-chip.
            Inactive: --dim color. */}
        <SettingsRow title="Theme">
          <div
            data-testid="theme-segmented"
            className="flex items-center gap-0.5 rounded-[var(--r-ctl)] border border-[var(--border)] bg-[var(--card)] p-0.5"
          >
            {(["Light", "Dark"] as const).map((label) => {
              const value = label.toLowerCase() as "light" | "dark";
              const selected = activeTheme === value;
              return (
                <button
                  key={value}
                  type="button"
                  aria-label={label}
                  aria-pressed={selected}
                  onClick={() => setPrefs({ theme: value })}
                  className={[
                    "rounded-[var(--r-chip)] px-3 py-1 text-[12px] font-medium transition-colors",
                    selected
                      ? "bg-[var(--raised)] text-[var(--text)]"
                      : "text-[var(--dim)] hover:text-[var(--text)]",
                  ].join(" ")}
                >
                  {label}
                </button>
              );
            })}
          </div>
        </SettingsRow>

        {/* Accent — §3.5 six-swatch picker.
            Each swatch renders its dark-base color as a filled circle.
            Selected swatch: 2px solid outline in --accent with 2px offset.
            Ring uses outline (not box-shadow) so it works on any bg.  */}
        <SettingsRow title="Accent color" fullWidth>
          <div
            data-testid="accent-picker"
            className="flex items-center gap-3"
          >
            {ACCENT_SWATCHES.map(({ id, label, color }) => {
              const selected = activeAccent === id;
              return (
                <button
                  key={id}
                  type="button"
                  aria-label={label}
                  aria-pressed={selected}
                  title={label}
                  onClick={() => setPrefs({ accent: id })}
                  className="relative rounded-full p-0 focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)] focus-visible:ring-offset-2"
                  style={selected
                    ? { outline: "2px solid var(--accent)", outlineOffset: "3px" }
                    : undefined}
                >
                  {/* Swatch circle — hex is the §3.5 dark-base preview color only */}
                  <span
                    aria-hidden="true"
                    className="block h-[22px] w-[22px] rounded-full"
                    style={{ background: color }}
                  />
                </button>
              );
            })}
          </div>
        </SettingsRow>

        {/* Translucency — §2 optional boolean; backdrop-blur / vibrancy on/off.
            Default true. Disable for solid backgrounds or accessibility needs. */}
        <SettingsRow
          title="Translucency"
          info={<InfoPopover text="Blur + transparency behind surfaces. Disable for solid backgrounds or low-end GPUs." />}
        >
          <Toggle
            aria-label="Translucency"
            checked={prefs.translucency ?? true}
            onChange={(v) => setPrefs({ translucency: v })}
          />
        </SettingsRow>

        {/* Mask sensitive data — §2 optional boolean; blur sensitive clipboard previews.
            Default true. Controls whether sensitive_spans ranges are redacted. */}
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
            formatValue={(v) => String(v)}
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
        {/* n9gp (PG-34): sensitive-reveal warning toggle — Android parity.
            Moved from Appearance panel (§2 removes all non-two-axis controls from
            Appearance) to History list where it governs display behavior. */}
        <SettingsRow
          title="Warn before revealing sensitive items"
          info={<InfoPopover text="Show a confirmation overlay before revealing blurred sensitive content. Matches the Android warning sheet behaviour. Turn off if you find the extra step redundant." />}
        >
          <Toggle
            checked={prefs.showSensitiveWarnings ?? true}
            onChange={(v) => setPrefs({ showSensitiveWarnings: v })}
          />
        </SettingsRow>
        {/* M5: historySize removed — history uses lazy pagination now */}
        {/* M6: previewDelay removed — replaced by explicit Eye preview button */}
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
            formatValue={(v) => String(v)}
          />
        </SettingsRow>
      </Panel>
    </div>
  );
}
