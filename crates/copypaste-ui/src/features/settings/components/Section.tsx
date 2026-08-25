/** A settings pane long enough to scroll needs somewhere for the eye to stop:
 *  the Service tab's nine rows in one undifferentiated list meant a user
 *  looking for one of them read all nine. */
import type { ReactNode } from "react";
import { SettingsGroupSurface } from "@/features/settings/components/SettingsGroupSurface";
import styles from "./Section.module.css";

interface SectionProps {
  title: string;
  description?: string;
  children: ReactNode;
}

export function Section({ title, description, children }: SectionProps) {
  return (
    <section data-settings-search-target={`section:${title}`} className={styles.root}>
      <div className={styles.heading}>
        <h2 className={styles.title}>{title}</h2>
        {description !== undefined && (
          <p className={styles.description}>{description}</p>
        )}
      </div>
      <SettingsGroupSurface>{children}</SettingsGroupSurface>
    </section>
  );
}
