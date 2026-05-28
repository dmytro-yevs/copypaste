import { useUI, type ViewId } from "../store";

// ---------------------------------------------------------------------------
// Lucide/Feather-style line icons — 1.5px stroke, currentColor, 16×16 viewport
// ---------------------------------------------------------------------------

// History: clock with a counter-clockwise arrow
function IconHistory({ active }: { active: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="15"
      height="15"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      // Blue — matches ide-accent
      className={active ? "text-white" : "text-ide-accent"}
    >
      {/* Clock face */}
      <circle cx="12" cy="12" r="9" />
      {/* Clock hands */}
      <polyline points="12 7 12 12 15 15" />
      {/* Counter-clockwise arrow on top-left of circle */}
      <path d="M3.05 11A9 9 0 0 1 6 5.7" />
      <polyline points="3 5 3 11 9 11" />
    </svg>
  );
}

// Devices: monitor + small phone (monitor-smartphone style)
function IconDevices({ active }: { active: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="15"
      height="15"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      // Teal
      className={active ? "text-white" : "text-[#56b6c2]"}
    >
      {/* Monitor */}
      <rect x="2" y="4" width="14" height="10" rx="2" />
      <path d="M8 18h2m-3 0h6" />
      <path d="M9 14v4" />
      {/* Phone */}
      <rect x="17" y="9" width="5" height="9" rx="1" />
      <line x1="19.5" y1="16.5" x2="19.5" y2="16.5" strokeWidth="2" strokeLinecap="round" />
    </svg>
  );
}

// Settings: classic gear / cog
function IconSettings({ active }: { active: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="15"
      height="15"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      // Amber — ide-warning
      className={active ? "text-white" : "text-[#d9a343]"}
    >
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  );
}

// About: info circle with "i" mark
function IconAbout({ active }: { active: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="15"
      height="15"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      // Grey — ide-dim
      className={active ? "text-white" : "text-ide-dim"}
    >
      <circle cx="12" cy="12" r="10" />
      {/* Dot */}
      <line x1="12" y1="8" x2="12" y2="8" strokeWidth="2" strokeLinecap="round" />
      {/* Stem */}
      <line x1="12" y1="12" x2="12" y2="16" />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Nav items
// ---------------------------------------------------------------------------

type NavItem = {
  id: ViewId;
  label: string;
  Icon: ({ active }: { active: boolean }) => React.ReactNode;
};

const NAV: NavItem[] = [
  { id: "history",  label: "History",  Icon: IconHistory },
  { id: "devices",  label: "Devices",  Icon: IconDevices },
  { id: "settings", label: "Settings", Icon: IconSettings },
  { id: "about",    label: "About",    Icon: IconAbout },
];

// ---------------------------------------------------------------------------
// Sidebar
// ---------------------------------------------------------------------------

export function Sidebar() {
  const view = useUI((s) => s.view);
  const setView = useUI((s) => s.setView);

  return (
    <aside className="flex w-52 shrink-0 flex-col border-r border-ide-border bg-ide-panel/60">
      {/* Drag region aligned with the macOS traffic lights. */}
      <div data-tauri-drag-region className="h-9 shrink-0" />
      <nav className="flex flex-col gap-0.5 px-2 py-2">
        {NAV.map(({ id, label, Icon }) => {
          const active = view === id;
          return (
            <button
              key={id}
              onClick={() => setView(id)}
              className={[
                "flex items-center gap-2.5 rounded-ide px-2.5 py-1.5 text-left text-[13px] transition-colors",
                active
                  ? "bg-ide-selection text-white"
                  : "text-ide-dim hover:bg-ide-hover hover:text-ide-text",
              ].join(" ")}
            >
              <span className="flex w-4 shrink-0 items-center justify-center">
                <Icon active={active} />
              </span>
              <span>{label}</span>
            </button>
          );
        })}
      </nav>
      <div className="mt-auto px-3 py-2 text-[11px] text-ide-faint">CopyPaste</div>
    </aside>
  );
}
