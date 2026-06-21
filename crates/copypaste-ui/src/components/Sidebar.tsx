import { useUI, type ViewId } from "../store";
import { SyncStatusChip } from "./SyncStatusChip";
import {
  HistoryIcon,
  DevicesIcon,
  SettingsIcon,
  AboutIcon,
  LogsIcon,
} from "./NavIcons";

// ---------------------------------------------------------------------------
// Nav items — SF-like stroke icons at 18×18 (§dt5k, 24px grid → rendered 18px)
// strokeWidth 1.85, fill none, stroke currentColor, linecap/join round.
// ---------------------------------------------------------------------------

type NavItem = {
  id: ViewId;
  label: string;
  Icon: React.ComponentType<{ className?: string }>;
};

// §jxbx: All nav items use the same icon/text classes driven by active state.
// Active: differs by skin — classic: fill+glow, quiet: tint, vapor: glass+ring.
// Inactive: muted ide-dim text; hover shows surface bg tint (no translateX: MOT-16).
const NAV: NavItem[] = [
  { id: "history",  label: "History",  Icon: HistoryIcon  },
  { id: "devices",  label: "Devices",  Icon: DevicesIcon  },
  { id: "settings", label: "Settings", Icon: SettingsIcon },
  { id: "about",    label: "About",    Icon: AboutIcon    },
  { id: "logs",     label: "Logs",     Icon: LogsIcon     },
];

// Stagger delay base + step for list-item-in entrance (ref: styleguide §nav-item anim-delay)
// Each item gets  180ms base + i * 55ms  — matches the SG formula exactly.
const STAGGER_BASE_MS = 180;
const STAGGER_STEP_MS = 55;

// ---------------------------------------------------------------------------
// Sidebar — v0.6 Liquid Glass: glass panel + radial accent tint + entrance anim.
// W-C1: Active nav styling branches by skin.
//   classic → fill+glow  (accent gradient bg + nav-active-glow breathing)
//   quiet   → tint        (accent-tint bg, no glow, no gradient)
//   vapor   → glass+ring  (surface-glass material + ring outline, no fill gradient)
// Structural dimensions (radius, shadow) read from --skin-* CSS custom properties
// via inline styles so they update on skin switch without component changes.
// ---------------------------------------------------------------------------

export function Sidebar() {
  const view = useUI((s) => s.view);
  const setView = useUI((s) => s.setView);
  // W-C1: read active skin to branch nav-active styling.
  const skin = useUI((s) => s.prefs.skin);

  return (
    <aside
      className={[
        // Glass panel with card-in entrance (§jxbx-1).
        // surface-glass provides the §3 translucency recipe.
        // card-in: cubic-bezier spring entrance from index.css utility.
        "surface-glass card-in",
        "flex w-[208px] shrink-0 flex-col",
        // W-C1: border-radius driven by --skin-r-card (14px classic, 10px quiet, 16px vapor).
        // Applied via inline style below; no rounded-ide-lg hardcode.
        // W-C1: box-shadow driven by --skin-shadow-card (e2 classic, none quiet, none vapor).
        // Applied via inline style below; no shadow-ide-sm hardcode.
        // Relative so the radial tint pseudo-overlay is positioned correctly.
        "relative overflow-hidden",
      ].join(" ")}
      style={{
        // W-C1: radius + shadow read from skin tokens so they update per skin.
        // classic: 14px card radius, e2 shadow.
        // quiet:   10px card radius, no shadow (--skin-shadow-card: none → no-op via fallback).
        // vapor:   16px card radius, no card shadow (card sheen via sheen token instead).
        borderRadius: "var(--skin-r-card, 14px)",
        boxShadow: "var(--skin-shadow-card, var(--ide-e2))",
      }}
    >
      {/*
        Radial accent tint — soft glow in the top portion of the sidebar (§jxbx-1).
        Matches the styleguide: radial-gradient circle at 40% 10% with accent at 24%.
        Pure decorative; pointer-events none, aria-hidden.
        Uses a Tailwind arbitrary background to stay token-driven (no hex).
      */}
      <div
        data-accent-tint
        aria-hidden
        className="pointer-events-none absolute inset-0 z-0"
        style={{
          // bg built from CSS custom properties — no hex values.
          background:
            "radial-gradient(circle at 40% 10%, color-mix(in srgb, var(--accent) 24%, transparent), transparent 42%)",
        }}
      />

      {/* All content above the tint overlay */}
      <div className="relative z-10 flex flex-1 flex-col">
        {/*
          Drag region aligned with macOS traffic lights (h-9 = 36px).
          The floating sidebar top is draggable.
        */}
        <div data-tauri-drag-region className="h-9 shrink-0" />

        <nav className="flex flex-col gap-0.5 px-2 pb-2">
          {NAV.map(({ id, label, Icon }, i) => {
            const active = view === id;
            return (
              <button
                key={id}
                onClick={() => setView(id)}
                // W-C1: active nav style branches by skin.
                // list-item-in + animationDelay on EVERY item for stagger entrance.
                // W-C1: border-radius driven by --skin-r-ctl via inline style (no rounded-ide hardcode).
                className={[
                  "list-item-in",
                  "flex items-center gap-2.5 px-2.5 py-[7px] text-left text-[13px]",
                  "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ide-accent",
                  // Transitions: background, color, box-shadow use motion tokens (MOT-16: no transform).
                  "transition-[background,color,box-shadow]",
                  "duration-200",
                  active
                    ? buildActiveClass(skin)
                    : [
                        // §jxbx-3/4: inactive — dim text, hover surface bg + opacity tint.
                        // No translateX: items would appear to leave the sidebar (MOT-16).
                        "text-ide-dim",
                        "hover:bg-ide-hover hover:text-ide-text",
                      ].join(" "),
                ].join(" ")}
                style={{
                  // §jxbx-5: stagger animationDelay for list-item-in entrance.
                  animationDelay: `${STAGGER_BASE_MS + i * STAGGER_STEP_MS}ms`,
                  // W-C1: border-radius from skin token (no rounded-ide hardcode).
                  // classic: 9px, quiet: 7px, vapor: 12px.
                  borderRadius: "var(--skin-r-ctl, 9px)",
                }}
              >
                <span className="flex w-[18px] shrink-0 items-center justify-center">
                  {/* aria-hidden is set inside each NavIcon svg; className passes currentColor tint */}
                  <Icon className={active ? buildActiveIconClass(skin) : "text-ide-dim"} />
                </span>
                <span className={active ? "font-medium" : ""}>{label}</span>
              </button>
            );
          })}
        </nav>

        {/* Footer: app name + sync status chip */}
        <div className="mt-auto flex items-center justify-between px-3 py-2.5">
          {/* ide-faint is WCAG AA 4.5:1 on panel; drop the /60 opacity that was ~1.8:1 */}
          <span className="text-[10.5px] font-medium uppercase tracking-widest text-ide-faint">CopyPaste</span>
          <SyncStatusChip />
        </div>
      </div>
    </aside>
  );
}

// ---------------------------------------------------------------------------
// Skin-specific active-nav styling helpers (W-C1)
// ---------------------------------------------------------------------------

/**
 * Returns Tailwind class string for the active nav button per skin.
 *
 *   classic → fill+glow: accent gradient fill + breathing glow + white text + small shadow.
 *             Reproduces the pre-skin Liquid Glass look exactly (Classic is frozen).
 *   quiet   → tint: accent-tint background, no glow, on-accent text colour.
 *             Flat material, no shadow — matches §2.2 navActive:"tint".
 *   vapor   → glass+ring: surface-glass material + ring outline, no opaque fill gradient.
 *             §2.2 navActive:"glass+ring" — glass surface + accent ring on the button.
 */
function buildActiveClass(skin: "classic" | "quiet" | "vapor"): string {
  switch (skin) {
    case "quiet":
      // Quiet: solid accent-tint bg (bg-ide-accentDim), accent text, no glow.
      // §2.2 navActive:"tint" — a readable accent wash without glass blur.
      return [
        "bg-ide-accentDim",
        "text-ide-accent",
      ].join(" ");

    case "vapor":
      // Vapor: glass surface + accent ring outline, no fill gradient.
      // §2.2 navActive:"glass+ring" — surface-glass adds blur/fill; ring = accent border.
      // ring-1 + ring-ide-accent via Tailwind adds a 1px ring shadow.
      // CopyPaste-i3ia: text MUST be theme-adaptive, NOT white. surface-glass is a
      // translucent tint of the panel — near-white on the light theme — so white
      // text on it is invisible (the active label/icon vanished in vapor+light).
      // text-ide-text resolves to the readable foreground for the current theme
      // (dark on light, light on dark), staying legible on the glass in both.
      return [
        "surface-glass",
        "ring-1 ring-ide-accent",
        "text-ide-text",
      ].join(" ");

    case "classic":
    default:
      // Classic: §jxbx-2: accent gradient + breathing glow + on-accent text.
      // bg-gradient-to-br gives Tailwind the gradient direction;
      // from/to are set via inline style to stay token-driven (no hex).
      // Preserves the exact pre-skin Liquid Glass look (Classic frozen).
      return [
        "nav-active-glow",
        "bg-gradient-to-br from-ide-accent to-ide-accentDim",
        "text-white",
        "shadow-ide-xs",
      ].join(" ");
  }
}

/**
 * Returns the icon colour class for the active nav icon per skin.
 *
 *   classic → white (on accent gradient fill)
 *   quiet   → accent text (on tint wash, maintains readability)
 *   vapor   → ide-text (theme-adaptive; on near-white glass in light theme white
 *             would vanish — see CopyPaste-i3ia)
 */
function buildActiveIconClass(skin: "classic" | "quiet" | "vapor"): string {
  switch (skin) {
    case "quiet":
      return "text-ide-accent";
    case "vapor":
      // CopyPaste-i3ia: theme-adaptive — white icon vanished on light-theme glass.
      return "text-ide-text";
    case "classic":
    default:
      return "text-white";
  }
}
