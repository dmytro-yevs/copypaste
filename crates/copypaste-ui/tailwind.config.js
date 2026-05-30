/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // JetBrains "New UI" 2023+ — blue-cool near-blacks with deliberate elevation.
        // Each layer is ~3-4% lighter so depth is visible but not garish.
        ide: {
          // ── Backgrounds (darkest → lightest) ──────────────────────────────
          // bg:       main window / outermost chrome  (very dark blue-black)
          // panel:    sidebar, tool-window panels      (classic Darcula base, now blue-tinted)
          // elevated: cards, inputs, dropdowns         (clearly lifted above panel)
          bg: "#13141a",
          panel: "#1e1f26",
          elevated: "#272930",

          // ── Structural ────────────────────────────────────────────────────
          // border:   hairline between sibling panels  (low-contrast, barely visible)
          // divider:  section dividers inside a panel  (slightly stronger)
          border: "#2e2f38",
          divider: "#3a3b44",

          // ── Interactive states ────────────────────────────────────────────
          // hover:     row/button hover fill (cool dark, slightly lighter than panel)
          // selection: selected row / active tree node (deep accent-tinted blue)
          hover: "#252730",
          selection: "#253565",

          // ── Typography hierarchy ──────────────────────────────────────────
          // text:  primary readable content  (~WCAG AA on ide-panel bg)
          // dim:   secondary labels, placeholders, timestamps
          // faint: disabled text, watermarks, sub-sub labels
          text: "#e8eaed",
          dim: "#9496a1",
          faint: "#5c5e6a",

          // ── Brand accent (JetBrains blue) ─────────────────────────────────
          // accent:      default blue — buttons, links, active indicators
          // accentHover: slightly lighter for hover state
          accent: "#3592ff",
          accentHover: "#5aa8ff",

          // ── Semantic ──────────────────────────────────────────────────────
          danger:  "#e05c5c",
          success: "#5fad65",
          warning: "#d9a343"
        }
      },
      fontFamily: {
        sans: ["-apple-system", "BlinkMacSystemFont", "Inter", "Segoe UI", "sans-serif"],
        mono: ["ui-monospace", "SFMono-Regular", "JetBrains Mono", "Menlo", "monospace"]
      },
      borderRadius: {
        ide: "6px"
      },
      boxShadow: {
        // Subtle panel lift — cards, popovers, dropdowns.
        "ide-panel": "0 1px 4px rgba(0,0,0,0.45), 0 4px 16px rgba(0,0,0,0.35)",
        // Stronger elevation for modals / floating windows.
        "ide-popup":  "0 4px 24px rgba(0,0,0,0.65), 0 1px 4px rgba(0,0,0,0.4)",
        // Focus ring inset glow around interactive controls.
        "ide-focus":  "0 0 0 2px rgba(53,146,255,0.4)"
      },
      transitionDuration: {
        fast: "120ms"
      }
    }
  },
  plugins: []
};
