import { TabsList, TabsTrigger } from "@/components/ui";
import { SettingsTabIcon } from "@/features/settings/components/SettingsTabIcon";
import {
  type PreferenceSectionDefinition,
} from "@/features/settings/model/preferenceSections";
import styles from "./SettingsNavigation.module.css";

export type { PreferenceSection } from "@/features/settings/model/preferenceSections";

export function SettingsNavigation({
  sections,
}: {
  sections: readonly PreferenceSectionDefinition[];
}) {
  return (
    <TabsList
      variant="bare"
      aria-label="Preference sections"
      className={styles.root}
    >
      {sections.map((section) => {
        return (
          <TabsTrigger
            key={section.value}
            value={section.value}
            className={styles.item}
          >
            <SettingsTabIcon name={section.icon} />
            <span className={styles.label}>{section.label}</span>
          </TabsTrigger>
        );
      })}
    </TabsList>
  );
}
