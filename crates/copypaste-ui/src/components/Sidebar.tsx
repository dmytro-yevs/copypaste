import {
  Clock,
  Info,
  MonitorSmartphone,
  ScrollText,
  Settings,
  type LucideIcon,
} from "lucide-react";
import { useUI, type ViewId } from "../store";
import { SyncStatusChip } from "./SyncStatusChip";

// Redesign wiring (Slice 5 / CopyPaste-g27b.12): .sb / .sb__item / .sb__foot
// (shell.css). Gallery nav item wiring happens in slice 6 — do not add it here.

type NavItem = {
  id: ViewId;
  label: string;
  icon: LucideIcon;
};

const NAV: NavItem[] = [
  { id: "history",  label: "History",  icon: Clock },
  { id: "devices",  label: "Devices",  icon: MonitorSmartphone },
  { id: "settings", label: "Settings", icon: Settings },
  { id: "about",    label: "About",    icon: Info },
  { id: "logs",     label: "Logs",     icon: ScrollText },
];

export function Sidebar() {
  const view = useUI((s) => s.view);
  const setView = useUI((s) => s.setView);

  return (
    <aside>
      <nav className="sb" aria-label="Primary">
        <div data-tauri-drag-region />

        {NAV.map(({ id, label, icon: Icon }) => {
          const active = view === id;
          return (
            <button
              key={id}
              className={active ? "sb__item on" : "sb__item"}
              onClick={() => setView(id)}
              aria-current={active ? "page" : undefined}
            >
              <Icon aria-hidden="true" />
              <span>{label}</span>
            </button>
          );
        })}

        <div className="sb__foot">
          <span>CopyPaste</span>
          <SyncStatusChip />
        </div>
      </nav>
    </aside>
  );
}
