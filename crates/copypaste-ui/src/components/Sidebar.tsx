import { useUI, type ViewId } from "../store";
import { SyncStatusChip } from "./SyncStatusChip";

// Classnames stripped in design-demolition pass (CopyPaste-h1n3).
// Icon usages removed in the aggressive de-style pass (CopyPaste-3sys); NavIcons
// import dropped accordingly.

type NavItem = {
  id: ViewId;
  label: string;
};

const NAV: NavItem[] = [
  { id: "history",  label: "History"  },
  { id: "devices",  label: "Devices"  },
  { id: "settings", label: "Settings" },
  { id: "about",    label: "About"    },
  { id: "logs",     label: "Logs"     },
];

export function Sidebar() {
  const view = useUI((s) => s.view);
  const setView = useUI((s) => s.setView);

  return (
    <aside>
      {/* Radial accent tint — pure decorative */}
      <div
        aria-hidden
      />

      <div>
        <div data-tauri-drag-region />

        <nav>
          {NAV.map(({ id, label }) => {
            const active = view === id;
            return (
              <button
                key={id}
                onClick={() => setView(id)}
                aria-current={active ? "page" : undefined}
              >
                <span>{label}</span>
              </button>
            );
          })}
        </nav>

        <div>
          <span>CopyPaste</span>
          <SyncStatusChip />
        </div>
      </div>
    </aside>
  );
}
