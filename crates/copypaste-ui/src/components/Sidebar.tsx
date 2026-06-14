import { useUI, type ViewId } from "../store";
import { SyncStatusChip } from "./SyncStatusChip";

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
      className={active ? "text-white" : "text-ide-accent"}
    >
      <circle cx="12" cy="12" r="9" />
      <polyline points="12 7 12 12 15 15" />
      <path d="M3.05 11A9 9 0 0 1 6 5.7" />
      <polyline points="3 5 3 11 9 11" />
    </svg>
  );
}

// Devices: monitor + small phone
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
      className={active ? "text-white" : "text-[#56b6c2]"}
    >
      <rect x="2" y="4" width="14" height="10" rx="2" />
      <path d="M8 18h2m-3 0h6" />
      <path d="M9 14v4" />
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
      className={active ? "text-white" : "text-ide-warning"}
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
      className={active ? "text-white" : "text-ide-dim"}
    >
      <circle cx="12" cy="12" r="10" />
      <line x1="12" y1="8" x2="12" y2="8" strokeWidth="2" strokeLinecap="round" />
      <line x1="12" y1="12" x2="12" y2="16" />
    </svg>
  );
}

// Logs: terminal / scroll icon
function IconLogs({ active }: { active: boolean }) {
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
      className={active ? "text-white" : "text-ide-dim"}
    >
      <polyline points="4 17 10 11 4 5" />
      <line x1="12" y1="19" x2="20" y2="19" />
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
  { id: "logs",     label: "Logs",     Icon: IconLogs },
];

// ---------------------------------------------------------------------------
// Sidebar — v0.5.3 restyle: darker panel bg, accent-pill active state,
// hairline right border, subtle bottom brand label.
// ---------------------------------------------------------------------------

export function Sidebar() {
  const view = useUI((s) => s.view);
  const setView = useUI((s) => s.setView);

  return (
    <aside
      className={[
        // surface-glass applies the canonical §3 translucency recipe:
        // rgba(19,20,26,.72)+blur(30px)+saturate(180%). The sidebar's panel bg
        // overlaid on the OS vibrancy layer gives the same visual depth without
        // a bespoke rgba value.
        "surface-glass",
        "flex w-[188px] shrink-0 flex-col",
        "border-r border-ide-border",
        "shadow-ide-sm",
      ].join(" ")}
    >
      {/* Drag region aligned with the macOS traffic lights (h-9 = 36px). */}
      <div data-tauri-drag-region className="h-9 shrink-0" />

      <nav className="flex flex-col gap-0.5 px-2 pb-2">
        {NAV.map(({ id, label, Icon }) => {
          const active = view === id;
          return (
            <button
              key={id}
              onClick={() => setView(id)}
              className={[
                "flex items-center gap-2.5 rounded-ide px-2.5 py-[7px] text-left text-[13px]",
                "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ide-accent",
                active
                  ? "bg-ide-selection text-white shadow-ide-xs"
                  : "text-ide-dim hover:bg-ide-hover hover:text-ide-text",
              ].join(" ")}
            >
              <span className="flex w-4 shrink-0 items-center justify-center">
                <Icon active={active} />
              </span>
              <span className={active ? "font-medium" : ""}>{label}</span>
            </button>
          );
        })}
      </nav>
      {/* Footer: app name + sync status chip */}
      <div className="mt-auto flex items-center justify-between px-3 py-2.5">
        {/* ide-faint is WCAG AA 4.5:1 on panel; drop the /60 opacity that was bringing it to ~1.8:1 */}
        <span className="text-[10px] font-medium uppercase tracking-widest text-ide-faint">CopyPaste</span>
        <SyncStatusChip />
      </div>
    </aside>
  );
}
