/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // ── Design System v2 "Quiet Precision" — canonical palette §0 ──────────
        // Single source of truth: web + Android must mirror these exact values.
        ide: {
          // Surface hierarchy: bg → panel → elevated → raised
          bg:        "#13141A",   // §0 canonical bg
          panel:     "#1B1C22",   // §0 canonical panel
          elevated:  "#23252D",   // §0 canonical elevated
          raised:    "#2d2f34",   // pressed / hover-on-elevated depth tier

          // Borders & dividers — hairline single-pixel style
          border:    "#383b42",   // outline borders
          divider:   "#2e3035",   // row separators, subtler than border

          // Interaction states (tokenised — §3 kill bg-white/10 magic)
          selection: "rgba(61,139,255,0.16)", // selected row fill
          hover:     "rgba(255,255,255,0.045)", // panel surface hover
          pressed:   "rgba(255,255,255,0.07)",  // press state

          // Text hierarchy
          text:  "#E8EAED",       // §0 canonical text
          dim:   "#9da0a8",       // secondary
          faint: "#6b6f78",       // timestamps, placeholders

          // Brand
          accent:      "#3D8BFF", // §0 canonical accent
          accentHover: "#5aacff", // hover state
          accentDim:   "rgba(61,139,255,0.12)", // accent container tint

          // Semantic colours (§3) — base / container @~10-12%
          danger:      "#E05C5C",
          dangerDim:   "rgba(224,92,92,0.10)",
          success:     "#5FAD65",
          successDim:  "rgba(95,173,101,0.10)",
          warning:     "#D9A343",
          warningDim:  "rgba(217,163,67,0.10)",
          info:        "#56B6C2",  // url / info
          infoDim:     "rgba(86,182,194,0.12)",
          violet:      "#C678DD",  // image / code
          violetDim:   "rgba(198,120,221,0.12)",
        }
      },
      fontFamily: {
        sans: ["-apple-system", "BlinkMacSystemFont", '"SF Pro Text"', '"Inter var"', "Inter", "system-ui", "sans-serif"],
        mono: ['"SF Mono"', "ui-monospace", '"JetBrains Mono"', "Menlo", "monospace"]
      },
      borderRadius: {
        ide:      "6px",   // inputs, buttons
        "ide-sm": "4px",   // chips, keycaps, highlights
        "ide-lg": "10px",  // cards
        "ide-xl": "14px",  // popup
      },
      boxShadow: {
        // Exact elevation levels §3 E0-E3
        "ide-e0": "none",
        "ide-e1": "0 1px 2px rgba(0,0,0,0.40), 0 0 0 1px rgba(255,255,255,0.04) inset",
        "ide-e2": "0 2px 8px rgba(0,0,0,0.45), 0 1px 2px rgba(0,0,0,0.35)",
        "ide-e3": "0 12px 40px rgba(0,0,0,0.55), 0 2px 8px rgba(0,0,0,0.40), inset 0 1px 0 rgba(255,255,255,0.06)",
        // Legacy aliases — keep so no existing class breaks
        "ide-xs":    "0 1px 2px rgba(0,0,0,0.40)",
        "ide-sm":    "0 2px 8px rgba(0,0,0,0.45), 0 1px 2px rgba(0,0,0,0.35)",
        "ide-md":    "0 4px 14px rgba(0,0,0,0.55), 0 2px 4px rgba(0,0,0,0.38)",
        "ide-popup": "0 12px 40px rgba(0,0,0,0.55), 0 2px 8px rgba(0,0,0,0.40), inset 0 1px 0 rgba(255,255,255,0.06)",
      },
      // Motion tokens §8 — 4 durations
      transitionDuration: {
        "instant": "90ms",
        "fast":    "130ms",
        "base":    "180ms",
        "slow":    "240ms",
        // Legacy
        ide: "120ms",
      },
      transitionTimingFunction: {
        // §8 eases
        "out-expo":  "cubic-bezier(.16,1,.3,1)",
        "standard":  "cubic-bezier(.2,0,0,1)",
        "in-curve":  "cubic-bezier(.4,0,1,1)",
        // Legacy
        ide: "ease",
      },
      keyframes: {
        // Popup entrance §4: scale .97→1 + opacity + translateY 4→0, 160ms out-expo
        popupEnter: {
          "0%":   { opacity: "0", transform: "scale(0.97) translateY(4px)" },
          "100%": { opacity: "1", transform: "scale(1) translateY(0)" },
        },
        // Toast slide-up §8
        toastIn: {
          "0%":   { opacity: "0", transform: "translateX(-50%) translateY(8px)" },
          "100%": { opacity: "1", transform: "translateX(-50%) translateY(0)" },
        },
        fadeIn: {
          "0%":   { opacity: "0", transform: "translateY(2px)" },
          "100%": { opacity: "1", transform: "translateY(0)" },
        },
        // Online pulse ring §7
        pulsePing: {
          "0%":   { transform: "scale(1)", opacity: "1" },
          "75%, 100%": { transform: "scale(2.2)", opacity: "0" },
        },
      },
      animation: {
        "popup-enter": "popupEnter 160ms cubic-bezier(.16,1,.3,1) both",
        "toast-in":    "toastIn 180ms cubic-bezier(.16,1,.3,1) both",
        "fade-in":     "fadeIn 130ms ease both",
        "pulse-ping":  "pulsePing 2s cubic-bezier(0,0,0.2,1) infinite",
      },
    }
  },
  plugins: []
};
