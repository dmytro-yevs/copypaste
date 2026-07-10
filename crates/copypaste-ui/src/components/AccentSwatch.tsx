// AccentSwatch.tsx
// Shared accent-color swatch button, extracted from DisplayTab.tsx (Appearance
// section, class "swatch"/shell.css) and GalleryView/index.tsx (accent
// switcher, class "gallery__swatch"/gallery.css) — same theme-scope +
// data-accent inline pattern, duplicated twice (CopyPaste-0j0p dedup).
//
// `className` stays a caller-supplied prop (not hardcoded here) so each call
// site keeps its own CSS class/size (20px fixed vs var(--icon-lg)) — unifying
// the sizes would be a visual change, out of scope for this dedup.
import type { AccentValue, ThemeValue } from "../lib/theme/prefsSchema";

export type AccentSwatchProps = {
  accent: AccentValue;
  selected: boolean;
  onSelect: (accent: AccentValue) => void;
  className: string;
  /** Only GalleryView's local theme switcher needs this; DisplayTab omits it
      (matches the pre-extraction behavior: no data-theme attribute there). */
  theme?: ThemeValue;
};

export function AccentSwatch({ accent, selected, onSelect, className, theme }: AccentSwatchProps) {
  return (
    <span className="theme-scope" data-theme={theme} data-accent={accent}>
      <button
        type="button"
        className={className}
        aria-label={accent}
        aria-pressed={selected}
        style={{ background: "var(--accent)" }}
        onClick={() => onSelect(accent)}
      />
    </span>
  );
}
