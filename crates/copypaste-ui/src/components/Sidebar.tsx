import {
  Clock,
  FlaskConical,
  MonitorSmartphone,
  Settings,
  type LucideIcon,
} from "lucide-react";
import { useUI, type ViewId } from "../store";
import { MOCK } from "../lib/ipc";
import { SyncStatusChip } from "./SyncStatusChip";

// Redesign wiring (Slice 5 / CopyPaste-g27b.12): .sb / .sb__item / .sb__foot
// (shell.css). The dev-only Gallery nav item (task 6.4) is wired below,
// outside the production NAV/ViewId registry — see navigateToGallery().

type NavItem = {
  id: ViewId;
  label: string;
  icon: LucideIcon;
};

const NAV: NavItem[] = [
  { id: "history",  label: "History",  icon: Clock },
  { id: "devices",  label: "Devices",  icon: MonitorSmartphone },
  { id: "settings", label: "Settings", icon: Settings },
];

// Dev-only: activate the gallery branch (App.tsx's `galleryActive()`) by
// setting `?view=gallery` on the current URL and navigating there. The
// gallery branch is read fresh from the URL on every App render — it is NOT
// store state (design.md Decision 6) — so a full navigation is the simplest
// way to flip it reliably without adding routing plumbing to this dev-only
// affordance. Existing query params (e.g. `?mock=1`) are preserved.
function navigateToGallery(): void {
  const url = new URL(window.location.href);
  url.searchParams.set("view", "gallery");
  window.location.href = url.toString();
}

export function Sidebar() {
  const view = useUI((s) => s.view);
  const setView = useUI((s) => s.setView);

  return (
    // display:contents so the .sb nav is the direct flex child of .app and
    // stretches to full height (no empty gap below the footer).
    <aside style={{ display: "contents" }}>
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

        {/* Dev-only component gallery entry (design.md Decision 6, task 6.4).
            Gated on the SAME import.meta.env.DEV && MOCK check that already
            tree-shakes GalleryView's dynamic import (App.tsx) and mockIpc.ts
            (lib/ipc/transport.ts) out of production — this item never renders
            in a production build even if other DEV tooling leaks in. */}
        {import.meta.env.DEV && MOCK && (
          <button
            type="button"
            className="sb__item"
            onClick={navigateToGallery}
            data-testid="sidebar-gallery-item"
          >
            <FlaskConical aria-hidden="true" />
            <span>Gallery</span>
          </button>
        )}

        <div className="sb__foot">
          <span className="sb__foot-label">CopyPaste</span>
          <SyncStatusChip />
        </div>
      </nav>
    </aside>
  );
}
