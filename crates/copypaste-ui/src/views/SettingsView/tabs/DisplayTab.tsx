// DisplayTab.tsx — design-stripped (CopyPaste-h1n3)
// Appearance section (Theme, Accent, Translucency) removed.
// "Mask sensitive data" relocated into "History list" panel.
import { SectionHeader } from "../../../components/SectionHeader";
import { SettingsRow } from "../../../components/SettingsRow";
import { Toggle } from "../../../components/Toggle";
import { Panel } from "../../../components/Panel";
import { SliderRow } from "../../../components/SliderRow";
import { InfoPopover } from "../components/InfoPopover";
import type { UIPrefs } from "../../../store";

export type DisplayTabProps = {
  prefs: UIPrefs;
  setPrefs: (p: Partial<UIPrefs>) => void;
};

export function DisplayTab({ prefs, setPrefs }: DisplayTabProps) {
  return (
    <div>
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
            formatValue={(v) => String(v)}
          />
        </SettingsRow>
      </Panel>
    </div>
  );
}
