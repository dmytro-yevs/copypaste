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

// §9.11: Active = bg-ide-selection text-ide-text; inactive = text-ide-dim.
// No translateX: items would appear to leave the sidebar (MOT-16).
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
// Sidebar — glass panel + radial accent tint + entrance anim.
// ---------------------------------------------------------------------------

export function Sidebar() {
  const view = useUI((s) => s.view);
  const setView = useUI((s) => s.setView);

  return (
    <aside
      className={[
        // Glass panel with card-in entrance.
        "surface-glass card-in",
        "flex w-[208px] shrink-0 flex-col",
        "relative overflow-hidden",
      ].join(" ")}
      style={{
        borderRadius: "var(--r-card)",
        boxShadow: "var(--sh1)",
      }}
    >
      {/*
        Radial accent tint — soft glow in the top portion of the sidebar.
        Pure decorative; pointer-events none, aria-hidden.
      */}
      <div
        data-accent-tint
        aria-hidden
        className="pointer-events-none absolute inset-0 z-0"
        style={{
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
                // §9.11: active = selection bg + text; inactive = dim + hover surface.
                // list-item-in + animationDelay for stagger entrance.
                className={[
                  "list-item-in",
                  "flex items-center gap-2.5 px-2.5 py-[7px] text-left text-[13px]",
                  "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ide-accent",
                  "transition-[background,color,box-shadow]",
                  "duration-200",
                  active
                    ? "bg-ide-selection text-ide-text"
                    : "text-ide-dim hover:bg-ide-hover hover:text-ide-text",
                ].join(" ")}
                style={{
                  animationDelay: `${STAGGER_BASE_MS + i * STAGGER_STEP_MS}ms`,
                  borderRadius: "var(--r-ctl)",
                }}
              >
                <span className="flex w-[18px] shrink-0 items-center justify-center">
                  {/* aria-hidden is set inside each NavIcon svg; className passes currentColor tint */}
                  <Icon className={active ? "text-ide-text" : "text-ide-dim"} />
                </span>
                <span className={active ? "font-medium" : ""}>{label}</span>
              </button>
            );
          })}
        </nav>

        {/* Footer: app name + sync status chip */}
        <div className="mt-auto flex items-center justify-between px-3 py-2.5">
          {/* ide-faint is WCAG AA 4.5:1 on panel */}
          <span className="text-[10.5px] font-medium uppercase tracking-widest text-ide-faint">CopyPaste</span>
          <SyncStatusChip />
        </div>
      </div>
    </aside>
  );
}
