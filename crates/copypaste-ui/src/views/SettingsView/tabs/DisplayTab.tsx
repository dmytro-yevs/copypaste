// DisplayTab.tsx
// Extracted from SettingsView.tsx renderDisplay() (CopyPaste-g06m.14 split) — cut/paste only.
import { SectionHeader } from "../../../components/SectionHeader";
import { SettingsRow } from "../../../components/SettingsRow";
import { Toggle } from "../../../components/Toggle";
import { Panel } from "../../../components/Panel";
import { SliderRow } from "../../../components/SliderRow";
import { InfoPopover } from "../components/InfoPopover";
import { PALETTE_KEYS, PALETTES } from "../../../lib/liquid-tokens";
import { SKIN_IDS } from "../../../lib/skins";
import type { UIPrefs } from "../../../store";

export type DisplayTabProps = {
  prefs: UIPrefs;
  setPrefs: (p: Partial<UIPrefs>) => void;
};

export function DisplayTab({ prefs, setPrefs }: DisplayTabProps) {
  const activePalette = prefs.palette ?? "graphite-mist";
  const activeDensity = prefs.density ?? "compact";
  // W-F4: skin picker — read current value from store, set via setPrefs({skin}).
  const activeSkin = prefs.skin ?? "classic";

  return (
    <div className="space-y-2">
      {/* ── Appearance ──────────────────────────────────────────────────────
          CopyPaste-hn5v: full palette + density + theme controls.
          Palette picker re-themes the whole app live via App.tsx
          data-palette attribute sync (already wired). */}
      <SectionHeader label="Appearance" />
      <Panel>
        {/* Palette picker — grid of 10 swatches.
            bdac.105: converted raw <div> with hardcoded 12px/text-ide-dim/mb-2
            to SettingsRow with fullWidth so label typography is density-aware. */}
        <SettingsRow title="Color palette" fullWidth>
          <div
            data-testid="palette-picker"
            className="grid grid-cols-5 gap-2"
          >
            {PALETTE_KEYS.map((key) => {
              const def = PALETTES[key];
              const isActive = activePalette === key;
              return (
                <button
                  key={key}
                  type="button"
                  aria-label={def.name}
                  aria-pressed={isActive}
                  title={def.name}
                  onClick={() => setPrefs({ palette: key })}
                  className={[
                    "group relative flex flex-col items-center gap-1 rounded-[8px] p-1.5 transition-all",
                    isActive
                      ? "ring-2 ring-ide-accent ring-offset-1 ring-offset-transparent bg-ide-elevated shadow-ide-e1"
                      : "hover:bg-ide-faint/12",
                  ].join(" ")}
                >
                  {/* Swatch circle using the palette's accent colour inline */}
                  <span
                    className="h-7 w-7 rounded-full shadow-ide-xs"
                    style={{ background: def.accent }}
                    aria-hidden="true"
                  />
                  <span className="max-w-full truncate text-center text-[10.5px] leading-tight text-ide-dim group-hover:text-ide-text">
                    {def.name}
                  </span>
                </button>
              );
            })}
          </div>
        </SettingsRow>

        {/* W-F4: Skin picker — Visual style segmented control (Classic / Quiet / Vapor).
            Mirrors the density/theme segmented control pattern exactly.
            Labels are Title-Cased skin ids. Updates live via setPrefs({skin}). */}
        <SettingsRow title="Visual style">
          <div
            data-testid="skin-picker"
            className="flex items-center gap-0.5 rounded-[10px] border border-ide-border/30 bg-ide-mute/18 p-0.5"
          >
            {SKIN_IDS.map((id) => {
              const label = id.charAt(0).toUpperCase() + id.slice(1);
              return (
                <button
                  key={id}
                  type="button"
                  aria-label={label}
                  aria-pressed={activeSkin === id}
                  onClick={() => setPrefs({ skin: id })}
                  className={[
                    "rounded-[7px] px-2.5 py-1 text-[12px] transition-colors",
                    activeSkin === id
                      ? "bg-ide-elevated text-ide-accent shadow-ide-e1"
                      : "text-ide-dim hover:text-ide-text",
                  ].join(" ")}
                >
                  {label}
                </button>
              );
            })}
          </div>
        </SettingsRow>

        {/* Density segmented control — compact / comfortable / spacious */}
        <SettingsRow title="Row density">
          {/* bpax/itsu: styleguide §form-controls segmented control — mute/.18 group bg,
              selected = white/.90 + e1 shadow, 7px inner radius.
              itsu: hairline border + mute token (was faint, now mute per spec) */}
          <div className="flex items-center gap-0.5 rounded-[10px] border border-ide-border/30 bg-ide-mute/18 p-0.5">
            {(["compact", "comfortable", "spacious"] as const).map((opt) => (
              <button
                key={opt}
                type="button"
                aria-label={opt}
                onClick={() => setPrefs({ density: opt })}
                className={[
                  "rounded-[7px] px-2.5 py-1 text-[12px] capitalize transition-colors",
                  activeDensity === opt
                    ? "bg-ide-elevated text-ide-accent shadow-ide-e1"
                    : "text-ide-dim hover:text-ide-text",
                ].join(" ")}
              >
                {opt.charAt(0).toUpperCase() + opt.slice(1)}
              </button>
            ))}
          </div>
        </SettingsRow>

        {/* Color theme — matches styleguide §form-controls segmented control */}
        {/* bdac.104: InfoPopover moved to info= slot (label column) */}
        {/* bdac.107: description added for Color theme row */}
        <SettingsRow
          title="Color theme"
          description="Overrides the system appearance for this app only."
          info={<InfoPopover text="Light uses a warm-white surface palette with WCAG AA contrast. Dark uses the default Design System v2 palette. System follows your OS appearance." />}
        >
          {/* bpax/web parity (CopyPaste-7qy §0): Light / Dark / System segmented control.
              Styleguide §form-controls: mute/.18 bg, selected=white/.90+shadow, 7px radius.
              itsu: hairline border + mute token (was faint, now mute per spec) */}
          <div className="flex items-center gap-0.5 rounded-[10px] border border-ide-border/30 bg-ide-mute/18 p-0.5">
            {(["light", "dark", "system"] as const).map((opt) => {
              const selected = (prefs.theme ?? "dark") === opt;
              return (
                <button
                  key={opt}
                  type="button"
                  aria-label={opt}
                  onClick={() => setPrefs({ theme: opt })}
                  className={[
                    "rounded-[7px] px-2.5 py-1 text-[12px] capitalize transition-colors",
                    selected
                      ? "bg-ide-elevated text-ide-accent shadow-ide-e1"
                      : "text-ide-dim hover:text-ide-text",
                  ].join(" ")}
                >
                  {opt}
                </button>
              );
            })}
          </div>
        </SettingsRow>

        {/* Translucency — kept here so all visual appearance controls are together */}
        {/* bdac.104: InfoPopover moved to info= slot */}
        <SettingsRow
          title="Translucency"
          info={<InfoPopover text="Blur + transparency behind surfaces. Disable for solid backgrounds." />}
        >
          <Toggle
            checked={prefs.translucency ?? true}
            onChange={(v) => setPrefs({ translucency: v })}
          />
        </SettingsRow>

        {/* Reduce motion — switches aurora from cinematic to calm profile.
            "calm" slows the aurora (--speed: 1.45) and dims it (--motion-opacity: .55).
            OS prefers-reduced-motion still zeroes the aurora automatically via CSS. */}
        {/* bdac.104: InfoPopover moved to info= slot */}
        <SettingsRow
          title="Reduce motion"
          info={<InfoPopover text="Slow and dim the aurora background animation. The OS 'Reduce Motion' accessibility setting stops it entirely regardless of this toggle." />}
        >
          <Toggle
            checked={prefs.motionReduced ?? false}
            onChange={(v) => setPrefs({ motionReduced: v })}
          />
        </SettingsRow>

        {/* n9gp (PG-34): sensitive-reveal warning toggle — Android parity.
            When on (default), a "Sensitive — preview hidden · click to reveal" overlay appears
            before the blur is lifted. When off, clicking the blur reveals
            immediately without the extra confirmation step. */}
        {/* bdac.104: InfoPopover moved to info= slot */}
        <SettingsRow
          title="Warn before revealing sensitive items"
          info={<InfoPopover text="Show a confirmation overlay before revealing blurred sensitive content. Matches the Android warning sheet behaviour. Turn off if you find the extra step redundant." />}
        >
          <Toggle
            checked={prefs.showSensitiveWarnings ?? true}
            onChange={(v) => setPrefs({ showSensitiveWarnings: v })}
          />
        </SettingsRow>
      </Panel>

      <SectionHeader label="History list" />
      <Panel>
        {/* M4: split previewLines — main window has its own independent setting */}
        {/* bdac.104: InfoPopover moved to info= slot */}
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
            the history list and the popup. Moved here from "Popup appearance"
            so users looking for list image sizing find it in the list section. */}
        {/* bdac.104: InfoPopover moved to info= slot */}
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
        {/* bdac.91: Group by device — persists the sort mode chosen in the History toolbar.
            Android parity: Android Settings.kt:627 sortByDevice, default false. */}
        {/* bdac.104: InfoPopover moved to info= slot */}
        <SettingsRow
          title="Group by device"
          info={<InfoPopover text="Group clipboard items by the device they came from, with your device shown first. You can also toggle this from the History toolbar when multiple devices are paired." />}
        >
          <Toggle
            checked={prefs.sortByDevice ?? false}
            onChange={(v) => setPrefs({ sortByDevice: v })}
          />
        </SettingsRow>
        {/* M5: historySize removed — history uses lazy pagination now */}
        {/* M6: previewDelay removed — replaced by explicit Eye preview button */}
      </Panel>

      <SectionHeader label="Popup appearance" hint="How the popup looks when triggered." />
      <Panel>
        {/* M4: popup gets its own independent preview-lines setting */}
        {/* bdac.104: InfoPopover moved to info= slot */}
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
