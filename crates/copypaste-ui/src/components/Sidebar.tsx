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
// Active: on-accent text (white) + accent gradient + nav-active-glow breathing.
// Inactive: muted ide-dim text; hover lifts with translateX + surface bg tint.
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
// Active nav: accent gradient + nav-active-glow.  Inactive: smooth translateX hover.
// ---------------------------------------------------------------------------

export function Sidebar() {
  const view = useUI((s) => s.view);
  const setView = useUI((s) => s.setView);

  return (
    <aside
      className={[
        // Glass panel with card-in entrance (§jxbx-1).
        // surface-glass provides the §3 translucency recipe.
        // card-in: cubic-bezier spring entrance from index.css utility.
        "surface-glass card-in",
        "flex w-[208px] shrink-0 flex-col",
        "rounded-ide-lg",
        "shadow-ide-sm",
        // Relative so the radial tint pseudo-overlay is positioned correctly.
        "relative overflow-hidden",
      ].join(" ")}
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
                // §jxbx-2/3/4/5: active gets accent gradient + nav-active-glow;
                // inactive gets ide-dim text + hover:translate-x-1 + smooth transitions.
                // list-item-in + animationDelay on EVERY item for stagger entrance.
                className={[
                  "list-item-in",
                  "flex items-center gap-2.5 rounded-ide px-2.5 py-[7px] text-left text-[13px]",
                  "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ide-accent",
                  // Transitions: background, color, transform, box-shadow all use motion tokens.
                  // spring cubic-bezier for transform; ease-out for colour/bg.
                  "transition-[transform,background,color,box-shadow]",
                  "duration-200",
                  active
                    ? [
                        // §jxbx-2: accent gradient + breathing glow + on-accent text.
                        // bg-gradient-to-br gives Tailwind the gradient direction;
                        // from/to are set via inline style to stay token-driven.
                        "nav-active-glow",
                        "bg-gradient-to-br from-ide-accent to-ide-accentDim",
                        "text-white",
                        "shadow-ide-xs",
                      ].join(" ")
                    : [
                        // §jxbx-3/4: inactive — dim text, hover surface bg + translateX(4px).
                        // hover:translate-x-1 = 4px in Tailwind default scale.
                        "text-ide-dim",
                        "hover:bg-ide-hover hover:text-ide-text",
                        "hover:translate-x-1",
                      ].join(" "),
                ].join(" ")}
                style={{
                  // §jxbx-5: stagger animationDelay for list-item-in entrance.
                  animationDelay: `${STAGGER_BASE_MS + i * STAGGER_STEP_MS}ms`,
                }}
              >
                <span className="flex w-[18px] shrink-0 items-center justify-center">
                  {/* aria-hidden is set inside each NavIcon svg; className passes currentColor tint */}
                  <Icon className={active ? "text-white" : "text-ide-dim"} />
                </span>
                <span className={active ? "font-medium" : ""}>{label}</span>
              </button>
            );
          })}
        </nav>

        {/* Footer: app name + sync status chip */}
        <div className="mt-auto flex items-center justify-between px-3 py-2.5">
          {/* ide-faint is WCAG AA 4.5:1 on panel; drop the /60 opacity that was ~1.8:1 */}
          <span className="text-[10px] font-medium uppercase tracking-widest text-ide-faint">CopyPaste</span>
          <SyncStatusChip />
        </div>
      </div>
    </aside>
  );
}
