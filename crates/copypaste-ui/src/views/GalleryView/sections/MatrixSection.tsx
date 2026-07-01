import {
  ACCENT_VALUES,
  THEME_VALUES,
  type AccentValue,
  type ThemeValue,
} from "../../../lib/theme/prefsSchema";

// Task 6.10 (design.md Decision 7/G2): the compact "token/critical-component
// matrix" — all 12 theme×accent combinations, but ONLY for a small critical
// subset (button, card, focus ring, status banner), not twelve full
// interactive app copies. Each cell is its own nested `.theme-scope` so
// var(--accent)/var(--bg)/etc resolve to THAT cell's theme/accent — never the
// gallery wrapper's (same technique the accent-swatch switcher already uses).
export function MatrixSection() {
  const cells: Array<{ theme: ThemeValue; accent: AccentValue }> = [];
  for (const theme of THEME_VALUES) {
    for (const accent of ACCENT_VALUES) {
      cells.push({ theme, accent });
    }
  }

  return (
    <section id="gallery-matrix">
      <h2>Token / critical-component matrix — all 12 theme × accent combinations</h2>
      <div className="gallery__matrix">
        {cells.map(({ theme, accent }) => (
          <div
            key={`${theme}-${accent}`}
            className="theme-scope gallery__matrix-cell"
            data-theme={theme}
            data-accent={accent}
            data-translucency="on"
          >
            <div className="gallery__matrix-label">
              {theme} · {accent}
            </div>
            <button type="button" className="btn btn--primary sm">
              Button
            </button>
            <div className="card gallery__matrix-card">Card</div>
            <button
              type="button"
              className="btn btn--secondary sm"
              data-force-state="focus"
            >
              Focus
            </button>
            <div className="banner banner--warn gallery__matrix-banner">
              <span className="banner__x">Warning</span>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
