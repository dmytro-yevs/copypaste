/**
 * The nav rail.
 *
 * `<nav>` is the Primary landmark (manifest §3.0); the active item carries
 * `aria-current="page"`. It is a list of links-as-buttons rather than a
 * `role="tablist"`, because these are screens and not tabs — the tab semantics
 * belong to Settings' sub-navigation, where Radix provides them.
 *
 * A11Y-15: at the 720px minimum the rail collapses to icons and the labels are
 * carried by `aria-label` + `title`, so nothing is ever hidden behind a
 * scroller.
 */
import { Clipboard, MonitorSmartphone, Settings2 } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { cn } from "@/lib/cn";
import { type View, useUi } from "@/store/ui";
import { StatusChip } from "@/components/shell/StatusChip";

const ITEMS: ReadonlyArray<{ view: View; label: string; icon: LucideIcon }> = [
  { view: "history", label: "History", icon: Clipboard },
  { view: "devices", label: "Devices", icon: MonitorSmartphone },
  { view: "settings", label: "Settings", icon: Settings2 },
];

export function Sidebar() {
  const view = useUi((s) => s.view);
  const setView = useUi((s) => s.setView);

  return (
    <nav
      aria-label="Primary"
      className="flex shrink-0 flex-col gap-s-1 border-r border-sidebar-border bg-sidebar p-s-2 sm:w-[var(--sidebar-w)]"
    >
      <div
        data-tauri-drag-region
        className="h-s-6 shrink-0"
        aria-hidden="true"
      />

      <ul className="flex flex-col gap-s-1">
        {ITEMS.map(({ view: id, label, icon: Icon }) => {
          const active = view === id;
          return (
            <li key={id}>
              <button
                type="button"
                onClick={() => setView(id)}
                aria-current={active ? "page" : undefined}
                title={label}
                className={cn(
                  "flex w-full items-center gap-s-2 rounded-md px-s-2 py-s-2 text-sm font-medium transition-colors duration-[var(--dur-fast)] outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50",
                  active
                    ? "bg-sidebar-accent text-sidebar-accent-foreground"
                    : "text-sidebar-foreground hover:bg-accent hover:text-accent-foreground",
                )}
              >
                <Icon size={16} aria-hidden="true" className="shrink-0" />
                <span className="hidden truncate sm:inline">{label}</span>
              </button>
            </li>
          );
        })}
      </ul>

      <div className="mt-auto hidden sm:block">
        <StatusChip />
      </div>
    </nav>
  );
}
