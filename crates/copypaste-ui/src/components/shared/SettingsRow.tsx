/**
 * A badge is a property of the field. A note reports its current state below
 * the row. Explanatory copy belongs in the optional popover so controls remain
 * aligned with their labels at narrow widths.
 */
import type { ReactNode } from "react";

import { HelpPopover } from "./HelpPopover";
import styles from "./SettingsRow.module.css";

interface SettingsRowProps {
  title: string;
  help?: ReactNode;
  helpLabel?: string;
  badge?: ReactNode;
  note?: ReactNode;
  children: ReactNode;
}

export function SettingsRow({
  title,
  help,
  helpLabel,
  badge,
  note,
  children,
}: SettingsRowProps) {
  return (
    <div
      data-settings-search-target={`row:${title}`}
      className={styles.root}
    >
      <div className={styles.copy}>
        <span className={styles.title}>
          <span>{title}</span>
          {help ? (
            <HelpPopover content={help} label={helpLabel ?? `More about ${title}`} />
          ) : null}
          {badge}
        </span>
        {note}
      </div>
      <div className={styles.control} data-settings-control>{children}</div>
    </div>
  );
}
