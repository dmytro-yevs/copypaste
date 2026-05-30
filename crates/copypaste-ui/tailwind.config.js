/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // JetBrains "New UI" v0.5.3 — deeper near-black ramp, #3592ff accent.
        // Token names are unchanged so no component edits are needed for the
        // base palette swap — only values change here.
        ide: {
          // Surface hierarchy: bg → panel → elevated → raised
          bg:        "#16171a",   // root window / darkest layer
          panel:     "#1e2024",   // primary surface: sidebar, list bg
          elevated:  "#26282d",   // cards, inputs
          raised:    "#2d2f34",   // hover / pressed on elevated — new depth tier

          // Borders & dividers — hairline single-pixel style
          border:    "#383b42",   // outline borders
          divider:   "#2e3035",   // row separators, subtler than border

          // Interaction states
          selection: "#1e3d72",   // selected row — deeper blue tint
          hover:     "#22252a",   // hover on panel surface

          // Text hierarchy
          text:  "#dfe1e5",       // primary
          dim:   "#9da0a8",       // secondary
          faint: "#6b6f78",       // timestamps, placeholders

          // Brand / semantic
          accent:      "#3592ff", // v0.5.3 brighter blue
          accentHover: "#5aacff", // hover state
          accentDim:   "#1a3661", // accent background tint — for badges, selection bg
          danger:      "#f07171", // slightly brighter danger
          success:     "#63c174", // slightly brighter success
          warning:     "#e5a93a", // amber
          warningDim:  "#3a2900", // warning surface tint — for pinned rows
        }
      },
      fontFamily: {
        sans: ["-apple-system", "BlinkMacSystemFont", "Inter", "Segoe UI", "sans-serif"],
        mono: ["ui-monospace", "SFMono-Regular", "JetBrains Mono", "Menlo", "monospace"]
      },
      borderRadius: {
        ide:    "6px",
        "ide-lg": "10px",
      },
      boxShadow: {
        // Layered depth shadows — restrained for a productivity tool.
        "ide-xs":    "0 1px 2px rgba(0,0,0,0.40)",
        "ide-sm":    "0 2px 6px rgba(0,0,0,0.48), 0 1px 2px rgba(0,0,0,0.32)",
        "ide-md":    "0 4px 14px rgba(0,0,0,0.55), 0 2px 4px rgba(0,0,0,0.38)",
        "ide-popup": "0 8px 28px rgba(0,0,0,0.68), 0 2px 6px rgba(0,0,0,0.44)",
      },
      transitionDuration: {
        ide: "120ms",
      },
      transitionTimingFunction: {
        ide: "ease",
      },
      keyframes: {
        fadeIn: {
          "0%":   { opacity: "0", transform: "translateY(2px)" },
          "100%": { opacity: "1", transform: "translateY(0)" },
        },
      },
      animation: {
        "fade-in": "fadeIn 120ms ease",
      },
    }
  },
  plugins: []
};
