// TabBar.tsx — extracted from SettingsView.tsx (CopyPaste-g06m.35)
// Animated sliding tab underline bar for the Settings screen.
import { useEffect, useRef, useState } from "react";
import { Settings, Monitor, RefreshCw, Keyboard, Database, Info, ScrollText, type LucideIcon } from "lucide-react";
import { tabListKeyDown } from "../../../lib/a11y/tabListKeyDown";

// CopyPaste-44rq.30: "advanced" removed — was a "coming soon" stub with no real content.
// File a new feature issue when Advanced tab content is ready to ship.
export type TabId = "general" | "display" | "sync" | "shortcuts" | "storage" | "about" | "logs";

export const TABS: { id: TabId; label: string; icon: LucideIcon }[] = [
  { id: "general",   label: "General",   icon: Settings },
  { id: "display",   label: "Display",   icon: Monitor },
  { id: "sync",      label: "Sync",      icon: RefreshCw },
  { id: "shortcuts", label: "Shortcuts", icon: Keyboard },
  { id: "storage",   label: "Storage",   icon: Database },
  { id: "about",     label: "About",     icon: Info },
  { id: "logs",      label: "Logs",      icon: ScrollText },
];

export function TabBar({
  active,
  onChange,
}: {
  active: TabId;
  onChange: (id: TabId) => void;
}) {
  // §6.1: Animated sliding tab underline.
  // Each button gets a ref so we can measure its offsetLeft + offsetWidth for
  // the absolutely-positioned indicator span. We use equal-width assumption
  // fallback when refs haven't mounted yet.
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const [indicatorStyle, setIndicatorStyle] = useState<{ left: number; width: number }>({
    left: 0,
    width: 0,
  });

  // Recompute indicator position whenever active tab changes.
  useEffect(() => {
    const activeIdx = TABS.findIndex((t) => t.id === active);
    const btn = tabRefs.current[activeIdx];
    if (btn) {
      setIndicatorStyle({ left: btn.offsetLeft, width: btn.offsetWidth });
    }
  }, [active]);

  // task 2.11/5: arrow-key navigation for the role="tablist". Left/Right (and
  // Home/End) move selection via the shared tabListKeyDown factory — React
  // state (the `active` prop / onChange) stays the source of truth.
  const currentIdx = TABS.findIndex((t) => t.id === active);
  const handleKeyDown = tabListKeyDown({
    count: TABS.length,
    current: currentIdx < 0 ? 0 : currentIdx,
    onSelect: (index) => onChange(TABS[index].id),
  });

  return (
    <div role="tablist" className="set-tabs" onKeyDown={handleKeyDown}>
      {TABS.map((t, idx) => {
        const Icon = t.icon;
        const isActive = active === t.id;
        return (
          <button
            key={t.id}
            ref={(el) => { tabRefs.current[idx] = el; }}
            type="button"
            role="tab"
            aria-selected={isActive}
            className={isActive ? "set-tab on" : "set-tab"}
            id={`tab-${t.id}`}
            aria-controls={`tabpanel-${t.id}`}
            onClick={() => onChange(t.id)}
          >
            <Icon aria-hidden="true" />
            {t.label}
          </button>
        );
      })}
      {/* Indicator position is computed from the active tab's measured
          offsetLeft/offsetWidth (functional, kept) — visual styling removed;
          .set-tab.on's border-bottom (shell.css) now carries the active mark. */}
      <span
        aria-hidden="true"
        style={{
          left: indicatorStyle.left,
          width: indicatorStyle.width,
        }}
      />
    </div>
  );
}
