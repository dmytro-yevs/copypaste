/**
 * The compact settings ladder: an index of every section, and one subpage at a
 * time. It replaces a nine-item tab strip that no phone width could hold —
 * on API 36 its overflowing labels stole the taps of their neighbours, and
 * wrapping the strip only traded that for a row of chrome taller than the pane
 * below it (A11Y-15).
 */
import { ChevronRight } from "lucide-react";
import { useEffect, useRef } from "react";

import { useTranslation } from "@/i18n";
import type { SettingsGroup, SettingsTab } from "@/components/settings/settingsTabs";

export function SettingsIndex({
  groups,
  open,
  onOpen,
}: {
  groups: readonly SettingsGroup[];
  open: SettingsTab | null;
  onOpen: (value: SettingsTab["value"]) => void;
}) {
  const { t } = useTranslation();
  const rows = useRef(new Map<string, HTMLButtonElement>());
  const lastOpened = useRef<string | null>(null);

  // A11Y-6: leaving a subpage puts focus back on the row that opened it, not at
  // the top of the document.
  useEffect(() => {
    if (open !== null) {
      lastOpened.current = open.value;
      return;
    }
    const value = lastOpened.current;
    if (value === null) return;
    lastOpened.current = null;
    rows.current.get(value)?.focus();
  }, [open]);

  if (open !== null) {
    return (
      <section aria-labelledby="settings-page-title" className="flex flex-col gap-s-3">
        {open.render()}
      </section>
    );
  }

  return (
    <nav aria-label={t("settings.sections")} className="flex flex-col gap-s-4">
      {groups.map((group) => (
        <section key={group.label} aria-labelledby={`settings-group-${group.label}`}>
          <h2
            id={`settings-group-${group.label}`}
            className="px-s-2 pb-s-1 text-[11px] font-medium tracking-wide text-muted-foreground"
          >
            {t(group.label)}
          </h2>
          <ul className="flex flex-col gap-s-1">
            {group.tabs.map((tab) => {
              const Icon = tab.icon;
              return (
                <li key={tab.value}>
                  <button
                    type="button"
                    data-settings-index-item={tab.value}
                    ref={(element) => {
                      if (element) rows.current.set(tab.value, element);
                      else rows.current.delete(tab.value);
                    }}
                    onClick={() => onOpen(tab.value)}
                    className="flex w-full min-h-[var(--tap-min)] items-center gap-s-2 rounded-lg border border-transparent bg-panel px-s-3 py-s-2 text-left text-sm font-medium outline-none hover:bg-accent hover:text-accent-foreground focus-visible:ring-[3px] focus-visible:ring-ring"
                  >
                    <Icon size={18} aria-hidden="true" className="shrink-0" />
                    <span className="min-w-0 break-words">{t(tab.label)}</span>
                    <ChevronRight
                      size={16}
                      aria-hidden="true"
                      className="ml-auto shrink-0 text-muted-foreground"
                    />
                  </button>
                </li>
              );
            })}
          </ul>
        </section>
      ))}
    </nav>
  );
}
