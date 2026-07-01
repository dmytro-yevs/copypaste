// TabBar.tsx — extracted from SettingsView.tsx (CopyPaste-g06m.35)
// Animated sliding tab underline bar for the Settings screen.
import { useEffect, useRef, useState } from "react";

// CopyPaste-44rq.30: "advanced" removed — was a "coming soon" stub with no real content.
// File a new feature issue when Advanced tab content is ready to ship.
export type TabId = "general" | "display" | "sync" | "shortcuts" | "storage";

export const TABS: { id: TabId; label: string }[] = [
  { id: "general",   label: "General"   },
  { id: "display",   label: "Display"   },
  { id: "sync",      label: "Sync"      },
  { id: "shortcuts", label: "Shortcuts" },
  { id: "storage",   label: "Storage"   },
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

  return (
    <div role="tablist">
      {TABS.map((t, idx) => (
        <button
          key={t.id}
          ref={(el) => { tabRefs.current[idx] = el; }}
          type="button"
          role="tab"
          aria-selected={active === t.id}
          id={`tab-${t.id}`}
          aria-controls={`tabpanel-${t.id}`}
          onClick={() => onChange(t.id)}
        >
          {t.label}
        </button>
      ))}
      {/* Indicator position is computed from the active tab's measured
          offsetLeft/offsetWidth (functional, kept) — visual styling removed. */}
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
