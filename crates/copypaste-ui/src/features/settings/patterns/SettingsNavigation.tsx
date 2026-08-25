import { Button } from "@/components/ui";
import { SettingsTabIcon } from "@/features/settings/components/SettingsTabIcon";
import {
  type PreferenceSectionDefinition,
  type PreferenceSection,
} from "@/features/settings/model/preferenceSections";
import styles from "./SettingsNavigation.module.css";

export type { PreferenceSection } from "@/features/settings/model/preferenceSections";

export function SettingsNavigation({
  active,
  onSelect,
  sections,
}: {
  active: PreferenceSection;
  onSelect: (value: PreferenceSection) => void;
  sections: readonly PreferenceSectionDefinition[];
}) {
  return (
    <nav role="tablist" aria-label="Preference sections" className={styles.root}>
      {sections.map((section) => {
        const selected = section.value === active;
        return (
          <Button
            key={section.value}
            type="button"
            variant="ghost"
            size="sm"
            role="tab"
            className={styles.item}
            aria-selected={selected}
            onClick={() => onSelect(section.value)}
          >
            <SettingsTabIcon name={section.icon} />
            <span className={styles.label}>{section.label}</span>
          </Button>
        );
      })}
    </nav>
  );
}
