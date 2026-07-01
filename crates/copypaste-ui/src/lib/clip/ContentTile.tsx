import { type CSSProperties } from "react";
import type { NormalizedKind } from "./normalizeContentKind";
import { KIND_PRESENTATION } from "./kindPresentation";

export interface ContentTileProps {
  kind: NormalizedKind;
  /**
   * For `color` items, the actual CSS color to show as a swatch (e.g. the item's
   * hex). When absent, the color glyph is shown instead.
   */
  colorValue?: string;
  /** Optional thumbnail element for `image` items (e.g. <ImageThumb/>). */
  thumb?: React.ReactNode;
}

/**
 * Shared content-type tile (design.md Decision 8). Renders the glyph for a
 * normalized kind, tinted by the kind's content-type token; a color swatch for
 * `color`; and a thumbnail (or gradient placeholder) for `image`. Decorative —
 * `aria-hidden` — because the accessible label lives on the row/metadata.
 */
export function ContentTile({ kind, colorValue, thumb }: ContentTileProps) {
  const p = KIND_PRESENTATION[kind];
  const ctStyle = { "--ct": `var(${p.token})` } as CSSProperties;

  if (kind === "color" && colorValue) {
    return (
      <span
        className="tile tile--swatch"
        style={{ ...ctStyle, background: colorValue }}
        aria-hidden="true"
      />
    );
  }

  if (kind === "image") {
    return (
      <span className="tile tile--thumb" style={ctStyle} aria-hidden="true">
        {thumb}
      </span>
    );
  }

  const { Icon } = p;
  return (
    <span className="tile" style={ctStyle} aria-hidden="true">
      <Icon />
    </span>
  );
}
