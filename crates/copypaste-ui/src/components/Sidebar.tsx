import { History, MonitorSmartphone, Settings, Info, ScrollText } from "lucide-react";
import { useUI, type ViewId } from "../store";
import { SyncStatusChip } from "./SyncStatusChip";

// ---------------------------------------------------------------------------
// Nav items — lucide-react icons at 18×18 (§9 nav=18px), stroke 1.5, currentColor
// ---------------------------------------------------------------------------

type NavItem = {
  id: ViewId;
  label: string;
  Icon: React.ComponentType<{ size?: number; strokeWidth?: number; className?: string; "aria-hidden"?: boolean }>;
  activeClass: string;
  inactiveClass: string;
};

// §9: ALL inactive nav icons use a single muted text-ide-dim (no rainbow tints).
// Active item keeps the accent pill + white icon+text via activeClass.
const NAV: NavItem[] = [
  { id: "history",  label: "History",  Icon: History,           activeClass: "text-white", inactiveClass: "text-ide-dim" },
  { id: "devices",  label: "Devices",  Icon: MonitorSmartphone, activeClass: "text-white", inactiveClass: "text-ide-dim" },
  { id: "settings", label: "Settings", Icon: Settings,          activeClass: "text-white", inactiveClass: "text-ide-dim" },
  { id: "about",    label: "About",    Icon: Info,              activeClass: "text-white", inactiveClass: "text-ide-dim" },
  { id: "logs",     label: "Logs",     Icon: ScrollText,        activeClass: "text-white", inactiveClass: "text-ide-dim" },
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
        "flex w-[208px] shrink-0 flex-col",
        "border-r border-ide-border",
        "shadow-ide-sm",
      ].join(" ")}
    >
      {/* Drag region aligned with the macOS traffic lights (h-9 = 36px). */}
      <div data-tauri-drag-region className="h-9 shrink-0" />

      <nav className="flex flex-col gap-0.5 px-2 pb-2">
        {NAV.map(({ id, label, Icon, activeClass, inactiveClass }) => {
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
              <span className="flex w-[18px] shrink-0 items-center justify-center">
                <Icon
                  size={18}
                  strokeWidth={1.5}
                  className={active ? activeClass : inactiveClass}
                  aria-hidden={true}
                />
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
