/**
 * The bar adds `--inset-bottom` rather than assuming a gesture bar: `env()`
 * with no fallback resolves to nothing and silently voids the `calc()`, which
 * is why the inset is a token.
 */
import { Clipboard, MonitorSmartphone, Settings2 } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import { isAndroidPlatform } from "@/lib/platform";
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
  const android = isAndroidPlatform();

  return (
    <nav
      aria-label={t("nav.primary")}
      className={cn(
        "flex shrink-0 bg-sidebar",
        android
          ? "min-h-[var(--tabbar-h)] border-t border-sidebar-border pb-[var(--inset-bottom)]"
          : "w-[var(--sidebar-w)] flex-col gap-s-1 border-r border-sidebar-border p-s-2",
      )}
    >
      <ul className={cn("flex flex-1", !android && "flex-col gap-s-1")}>
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
                  "flex w-full items-center rounded-md px-s-2 py-s-2 font-medium outline-none transition-colors duration-[var(--dur-fast)] focus-visible:ring-[3px] focus-visible:ring-ring",
                  android
                    ? "min-h-[var(--tap-min)] flex-col justify-center gap-0.5 text-xs"
                    : "min-h-[var(--tap-min)] flex-row justify-start gap-s-2 text-sm",
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

      {!android && (
        <div className="mt-auto">
          <StatusChip />
        </div>
      )}
    </nav>
  );
}
