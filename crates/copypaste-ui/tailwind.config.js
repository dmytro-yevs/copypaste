/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // ── Apple macOS Tahoe "Liquid Glass" — themeable palette ───────────────
        // Solid colours reference the --ide-*-rgb CHANNEL triplets defined in
        // index.css, wrapped as rgb(... / <alpha-value>) so opacity modifiers
        // (bg-ide-panel/60) work AND the data-theme="light" cascade re-themes
        // every utility. Pre-composited alpha tokens (selection/hover/dim/ghost/
        // *Dim) map straight to their --ide-* var. Light overrides live in
        // index.css :root[data-theme="light"]. (docs/PARITY-SPEC.md §1)
        ide: {
          // Surface hierarchy: bg → panel → elevated → raised
          bg:        "rgb(var(--ide-bg-rgb) / <alpha-value>)",
          panel:     "rgb(var(--ide-panel-rgb) / <alpha-value>)",
          elevated:  "rgb(var(--ide-elevated-rgb) / <alpha-value>)",
          raised:    "rgb(var(--ide-raised-rgb) / <alpha-value>)",

          // Borders & dividers — hairline single-pixel style
          border:    "rgb(var(--ide-border-rgb) / <alpha-value>)",
          divider:   "rgb(var(--ide-divider-rgb) / <alpha-value>)",

          // Interaction states (pre-composited alpha — pass through)
          selection: "var(--ide-selection)",
          hover:     "var(--ide-hover)",
          pressed:   "var(--ide-pressed)",

          // Text hierarchy
          text:  "rgb(var(--ide-text-rgb) / <alpha-value>)",
          dim:   "rgb(var(--ide-dim-rgb) / <alpha-value>)",
          faint: "rgb(var(--ide-faint-rgb) / <alpha-value>)",
          // mute: lighter than faint — used for segmented-control fill background
          // (spec §form-controls mute@.18). Falls back to faint until index.css defines --ide-mute-rgb.
          mute:  "rgb(var(--ide-mute-rgb, var(--ide-faint-rgb)) / <alpha-value>)",
          ghost: "var(--ide-ghost)",
          "ghost-deco": "var(--ide-ghost-deco)",

          // Brand
          accent:      "rgb(var(--ide-accent-rgb) / <alpha-value>)",
          accentHover: "rgb(var(--ide-accent-hover-rgb) / <alpha-value>)",
          accentDim:   "var(--ide-accent-dim)",

          // Semantic colours (§3) — base channel + pre-composited container tint
          danger:      "rgb(var(--ide-danger-rgb) / <alpha-value>)",
          dangerDim:   "var(--ide-danger-dim)",
          success:     "rgb(var(--ide-success-rgb) / <alpha-value>)",
          successDim:  "var(--ide-success-dim)",
          warning:     "rgb(var(--ide-warning-rgb) / <alpha-value>)",
          warningDim:  "var(--ide-warning-dim)",
          info:        "rgb(var(--ide-info-rgb) / <alpha-value>)",
          infoDim:     "var(--ide-info-dim)",
          violet:      "rgb(var(--ide-violet-rgb) / <alpha-value>)",
          violetDim:   "var(--ide-violet-dim)",
          // 1hqt: sky token for URL/IMAGE kinds (light: 20 120 170, dark: same as info)
          sky:         "rgb(var(--ide-sky-rgb) / <alpha-value>)",
          skyDim:      "var(--ide-sky-dim)",
          // 8qzb: badge-warning (pinned amber #D9A343) — separate from warning text token
          "badge-warning": "rgb(var(--ide-badge-warning-rgb) / <alpha-value>)",
        }
      },
      fontFamily: {
        // Bundled Inter/JetBrains Mono lead → pixel-identical across macOS + Android.
        // System fonts trail as safe fallback when .woff2 drop-ins are absent.
        // (DESIGN-SYSTEM-v2 §1 / §10)
        sans: ["Inter", "-apple-system", "BlinkMacSystemFont", '"SF Pro Text"', "system-ui", "sans-serif"],
        mono: ['"JetBrains Mono"', '"SF Mono"', "ui-monospace", "Menlo", "monospace"],
      },
      borderRadius: {
        ide:      "9px",   // ix8u: inputs, buttons, controls — styleguide --radius-ctl 9px
        "ide-sm": "7px",   // ix8u: chips, keycaps — styleguide --radius-chip 7px
        "ide-lg": "14px",  // ix8u: cards + modals — styleguide --radius-card 14px
        "ide-xl": "14px",  // popup
      },
      boxShadow: {
        // Theme-aware elevation §3 E0-E3 — the actual shadow values live in
        // index.css (--ide-e*) and differ per theme (heavy dark drops vs soft
        // light drops), so light cards don't keep muddy dark shadows.
        "ide-e0": "var(--ide-e0)",
        "ide-e1": "var(--ide-e1)",
        "ide-e2": "var(--ide-e2)",
        "ide-e3": "var(--ide-e3)",
        // Legacy aliases → map onto the same theme-aware tiers
        "ide-xs":    "var(--ide-e1)",
        "ide-sm":    "var(--ide-e2)",
        "ide-md":    "var(--ide-e3)",
        "ide-popup": "var(--ide-e3)",
      },
      // Motion tokens §8 — 4 durations (legacy --motion-* parity)
      transitionDuration: {
        "instant": "90ms",
        "fast":    "130ms",
        "base":    "180ms",
        "slow":    "240ms",
        // §8 Approved motion language (--mo-* tokens)
        "mo-instant": "var(--mo-instant, 90ms)",
        "mo-fast":    "var(--mo-fast, 130ms)",
        "mo-base":    "var(--mo-base, 180ms)",
        "mo-slow":    "var(--mo-slow, 240ms)",
        // Legacy
        ide: "120ms",
      },
      transitionTimingFunction: {
        // §8 eases (legacy)
        "out-expo":  "cubic-bezier(.16,1,.3,1)",
        "standard":  "cubic-bezier(.2,0,0,1)",
        "in-curve":  "cubic-bezier(.4,0,1,1)",
        // §8 Approved motion language (--mo-ease-* tokens)
        "mo-standard":   "var(--mo-ease-standard, cubic-bezier(.2,0,.2,1))",
        "mo-emphasized": "var(--mo-ease-emphasized, cubic-bezier(.16,1,.3,1))",
        // Legacy
        ide: "ease",
      },
      keyframes: {
        // Popup entrance §4: scale .97→1 + opacity + translateY 4→0, 160ms out-expo
        popupEnter: {
          "0%":   { opacity: "0", transform: "scale(0.97) translateY(4px)" },
          "100%": { opacity: "1", transform: "scale(1) translateY(0)" },
        },
        // §MO-6 Toast enter/exit
        toastEnter: {
          "0%":   { opacity: "0", transform: "translateX(-50%) translateY(8px) scale(.985)" },
          "100%": { opacity: "1", transform: "translateX(-50%) translateY(0) scale(1)" },
        },
        toastExit: {
          "0%":   { opacity: "1", transform: "translateX(-50%) translateY(0) scale(1)" },
          "100%": { opacity: "0", transform: "translateX(-50%) translateY(7px) scale(.992)" },
        },
        // Legacy toast (kept for compat)
        fadeIn: {
          "0%":   { opacity: "0", transform: "translateY(2px)" },
          "100%": { opacity: "1", transform: "translateY(0)" },
        },
        // §MO-1 Modal scrim + card
        modalScrimIn:  { "0%": { opacity: "0" }, "100%": { opacity: "1" } },
        modalScrimOut: { "0%": { opacity: "1" }, "100%": { opacity: "0" } },
        modalCardIn: {
          "0%":   { opacity: "0", transform: "translateY(8px) scale(.985)" },
          "100%": { opacity: "1", transform: "translateY(0) scale(1)" },
        },
        modalCardOut: {
          "0%":   { opacity: "1", transform: "translateY(0) scale(1)" },
          "100%": { opacity: "0", transform: "translateY(8px) scale(.985)" },
        },
        // §MO-2 Page panel slide
        panelSlideInRight: {
          "0%":   { opacity: "0", transform: "translateX(20px)" },
          "100%": { opacity: "1", transform: "translateX(0)" },
        },
        panelSlideOutLeft: {
          "0%":   { opacity: "1", transform: "translateX(0)" },
          "100%": { opacity: "0", transform: "translateX(-20px)" },
        },
        // §MO-4 Copy-flash (instant, 90ms)
        copyFlash: {
          "0%":   { outlineColor: "transparent", backgroundColor: "transparent" },
          "45%":  { outlineColor: "var(--ide-success)", backgroundColor: "var(--ide-success-dim)" },
          "100%": { outlineColor: "transparent", backgroundColor: "transparent" },
        },
        // §MO-5 Online pulse ONE-SHOT (2s forwards — not infinite)
        onlinePulse: {
          "0%":   { boxShadow: "0 0 0 0 var(--ide-success)", opacity: "1" },
          "70%":  { boxShadow: "0 0 0 9px var(--ide-success)", opacity: "0" },
          "100%": { boxShadow: "0 0 0 9px transparent", opacity: "0" },
        },
        // §MO-3 Row press scale
        rowPress: {
          "0%":   { transform: "scale(1)" },
          "50%":  { transform: "scale(.992)" },
          "100%": { transform: "scale(1)" },
        },
        // auroraSpin removed — perpetual spin was "loud/gimmicky" per spec.
        // glowDrift removed — replaced with static ambient blob.
        // glassShine removed — replaced with permanent ::before diagonal sheen (VISM-5).
        // Premium entrance animations (CopyPaste-52mz) — spring cubic-bezier, NOT bounce
        cardIn: {
          "0%":   { opacity: "0", transform: "scale(.96) translateY(10px)" },
          "100%": { opacity: "1", transform: "scale(1) translateY(0)" },
        },
        revealUp: {
          "0%":   { opacity: "0", transform: "translateY(6px)" },
          "100%": { opacity: "1", transform: "translateY(0)" },
        },
        listItemIn: {
          "0%":   { opacity: "0", transform: "translateY(4px)" },
          "100%": { opacity: "1", transform: "translateY(0)" },
        },
      },
      animation: {
        "popup-enter":         "popupEnter 160ms cubic-bezier(.16,1,.3,1) both",
        // §MO-6 Toast
        "toast-enter":         "toastEnter 180ms cubic-bezier(.16,1,.3,1) both",
        "toast-exit":          "toastExit 180ms cubic-bezier(.16,1,.3,1) both",
        "fade-in":             "fadeIn 130ms ease both",
        // §MO-1 Modal
        "modal-scrim-enter":   "modalScrimIn 180ms cubic-bezier(.2,0,.2,1) both",
        "modal-scrim-exit":    "modalScrimOut 180ms cubic-bezier(.2,0,.2,1) both",
        "modal-card-enter":    "modalCardIn 240ms cubic-bezier(.16,1,.3,1) both",
        "modal-card-exit":     "modalCardOut 240ms cubic-bezier(.16,1,.3,1) both",
        // §MO-2 Page panel
        "panel-enter-right":   "panelSlideInRight 240ms cubic-bezier(.16,1,.3,1) both",
        "panel-exit-left":     "panelSlideOutLeft 240ms cubic-bezier(.16,1,.3,1) both",
        // §MO-4 Copy-flash — 180ms (--mo-base) to match CSS primitive + JS timer (crh3.20)
        "copy-flash":          "copyFlash 180ms cubic-bezier(.2,0,.2,1) both",
        // §MO-5 Online pulse (ONE-SHOT, not infinite)
        "online-pulse":        "onlinePulse 2s cubic-bezier(.2,0,.2,1) forwards",
        // §MO-3 Row press
        "row-press":           "rowPress 130ms cubic-bezier(.2,0,.2,1)",
        // aurora-spin, glow-drift, glass-shine REMOVED (loud/gimmicky per spec)
        // Entrance (CopyPaste-52mz)
        "card-in":       "cardIn 158ms cubic-bezier(.16,1,.3,1) both",    /* 220ms × 0.72 */
        "reveal-up":     "revealUp 130ms cubic-bezier(.16,1,.3,1) both",  /* 180ms × 0.72 */
        "list-item-in":  "listItemIn 101ms cubic-bezier(.16,1,.3,1) both", /* 140ms × 0.72 */
      },
    }
  },
  // SCRL-2: dynamically-composed left-border accent classes used in LogView.tsx
  // (LEVEL_BORDER map). Tailwind's static scanner cannot see these strings — they
  // are concatenated at runtime — so they must be safelisted to survive production
  // purging (bd CopyPaste-5917.81).
  safelist: [
    "border-l-ide-danger",
    "border-l-ide-warning",
  ],
  plugins: []
};
