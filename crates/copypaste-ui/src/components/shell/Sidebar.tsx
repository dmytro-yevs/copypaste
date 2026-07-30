/**
 * The bar adds `--inset-bottom` rather than assuming a gesture bar: `env()`
 * with no fallback resolves to nothing and silently voids the `calc()`, which
 * is why the inset is a token.
 */
import { Clipboard, MonitorSmartphone, Settings2 } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import { type View, useUi } from "@/store/ui";
import { StatusChip } from "@/components/shell/StatusChip";

const ITEMS = [
  { view: "history", label: "nav.history", icon: Clipboard },
  { view: "devices", label: "nav.devices", icon: MonitorSmartphone },
  { view: "settings", label: "nav.settings", icon: Settings2 },
] as const satisfies ReadonlyArray<{
  view: View;
  label: string;
  icon: LucideIcon;
}>;

export function Sidebar() {
  const { t } = useTranslation();
  const view = useUi((s) => s.view);
  const setView = useUi((s) => s.setView);

  return (
    <nav
      aria-label={t("nav.primary")}
      className={cn(
        "flex shrink-0 bg-sidebar",
        "border-t border-sidebar-border sm:border-t-0 sm:border-r",
        "min-h-[var(--tabbar-h)] pb-[var(--inset-bottom)] sm:min-h-0 sm:pb-0",
        "sm:w-[var(--sidebar-w)] sm:flex-col sm:gap-s-1 sm:p-s-2",
      )}
    >
      <div
        data-tauri-drag-region
        aria-hidden="true"
        className="hidden h-s-6 shrink-0 sm:block"
      />

      <ul className="flex flex-1 sm:flex-col sm:gap-s-1">
        {ITEMS.map(({ view: id, label: key, icon: Icon }) => {
          const active = view === id;
          const label = t(key);
          return (
            <li key={id} className="flex-1">
              <button
                type="button"
                onClick={() => setView(id)}
                aria-current={active ? "page" : undefined}
                title={label}
                className={cn(
                  "flex w-full flex-col items-center justify-center gap-0.5 rounded-md px-s-2 py-s-2 text-xs font-medium outline-none transition-colors duration-[var(--dur-fast)] focus-visible:ring-[3px] focus-visible:ring-ring",
                  "min-h-[var(--tap-min)] sm:flex-row sm:justify-start sm:gap-s-2 sm:text-sm",
                  active
                    ? "bg-sidebar-accent text-sidebar-accent-foreground"
                    : "text-sidebar-foreground hover:bg-accent hover:text-accent-foreground",
                )}
              >
                <Icon size={18} aria-hidden="true" className="shrink-0" />
                <span className="truncate">{label}</span>
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
