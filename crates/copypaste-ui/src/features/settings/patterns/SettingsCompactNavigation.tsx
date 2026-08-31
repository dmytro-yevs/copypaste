import { Icon } from "@/components/ui/icon";
import type { ReactNode } from "react";

import { ActionButton, ScreenHeader } from "@/components/shared";
import { Button } from "@/components/ui";
import { SettingsTabIcon } from "@/features/settings/components/SettingsTabIcon";
import {
  type PreferenceSectionDefinition,
  type PreferenceSection,
} from "@/features/settings/model/preferenceSections";
import styles from "./SettingsCompactNavigation.module.css";

export function SettingsCompactNavigation({
  active,
  onSelect,
  onBack,
  renderSection,
  sections,
}: {
  active: PreferenceSection | null;
  onSelect: (section: PreferenceSection) => void;
  onBack: () => void;
  renderSection: (section: PreferenceSection) => ReactNode;
  sections: readonly PreferenceSectionDefinition[];
}) {
  const definition = sections.find(
    (section) => section.value === active,
  );

  if (active && definition) {
    return (
      <section className={styles.detail} aria-label={definition.label}>
        <ScreenHeader
          className={styles.detailHeader}
          leading={<ActionButton
            size="compactIcon"
            variant="ghost"
            icon="back"
            aria-label="Back to Settings"
            onClick={onBack}
          />}
          title={definition.label}
          description={definition.description}
        />
        <div className={styles.detailContent}>{renderSection(active)}</div>
      </section>
    );
  }

  return (
    <nav className={styles.menu} aria-label="Settings sections">
      {sections.map((section) => (
        <Button
          key={section.value}
          type="button"
          variant="ghost"
          size="md"
          className={styles.category}
          onClick={() => onSelect(section.value)}
        >
          <span className={styles.iconBox} aria-hidden="true">
            <SettingsTabIcon name={section.icon} />
          </span>
          <span className={styles.copy}>
            <strong>{section.label}</strong>
            <span>{section.description}</span>
          </span>
          <Icon name="caretRight" size="sm" className={styles.caret} />
        </Button>
      ))}
    </nav>
  );
}
