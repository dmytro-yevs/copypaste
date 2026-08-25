import { Icon } from "@/components/ui/icon";
import { useEffect, useRef, type ReactNode } from "react";

import { Button, Surface } from "@/components/ui";
import { SettingsTabIcon } from "@/features/settings/components/SettingsTabIcon";
import type {
  SettingsGroup,
  SettingsTab,
  SettingsTabValue,
} from "@/features/settings/model/settingsNavigation";
import { useTranslation } from "@/i18n";
import styles from "./SettingsIndex.module.css";

interface SettingsIndexProps {
  groups: readonly SettingsGroup[];
  open: SettingsTab | null;
  summaries?: Readonly<Partial<Record<SettingsTabValue, string>>>;
  onOpen: (value: SettingsTab["value"]) => void;
  children?: ReactNode;
}

export function SettingsIndex({
  groups,
  open,
  summaries = {},
  onOpen,
  children,
}: SettingsIndexProps) {
  const { t } = useTranslation();
  const rows = useRef(new Map<string, HTMLButtonElement>());
  const lastOpened = useRef<string | null>(null);

  useEffect(() => {
    if (open !== null) {
      lastOpened.current = open.value;
      return;
    }
    const value = lastOpened.current;
    if (value === null) return;
    lastOpened.current = null;
    rows.current.get(value)?.focus({ preventScroll: true });
  }, [open]);

  if (open !== null) {
    return (
      <section aria-labelledby="settings-page-title" className={styles.page}>
        {children}
      </section>
    );
  }

  return (
    <nav aria-label={t("settings.sections")} className={styles.index}>
      {groups.map((group) => {
        const groupId = `settings-group-${group.label.replace(/\./g, "-")}`;
        return (
          <section key={group.label} aria-labelledby={groupId} className={styles.group}>
            <h2 id={groupId} className={styles.groupTitle}>{t(group.label)}</h2>
            <Surface asChild elevation="raised" border="subtle" radius="md">
              <ul className={styles.groupList}>
                {group.tabs.map((tab) => {
                  const summary = summaries[tab.value];
                  return (
                    <li key={tab.value}>
                      <Button
                        type="button"
                        variant="ghost"
                        size="md"
                        data-settings-index-item={tab.value}
                        ref={(element) => {
                          if (element) rows.current.set(tab.value, element);
                          else rows.current.delete(tab.value);
                        }}
                        onClick={() => onOpen(tab.value)}
                        className={styles.indexButton}
                      >
                        <span className={styles.leadingIcon} aria-hidden="true">
                          <SettingsTabIcon name={tab.icon} />
                        </span>
                        <span className={styles.copy}>
                          <span className={styles.label}>{t(tab.label)}</span>
                          {summary ? <span className={styles.summary}>{summary}</span> : null}
                        </span>
                        <Icon name="caretRight" size="sm" className={styles.chevron} />
                      </Button>
                    </li>
                  );
                })}
              </ul>
            </Surface>
          </section>
        );
      })}
    </nav>
  );
}
