/**
 * A badge is a property of the field. A note reports its current state below
 * the row. Explanatory copy belongs in the optional popover so controls remain
 * aligned with their labels at narrow widths.
 */
import * as PopoverPrimitive from "@radix-ui/react-popover";
import type { ReactNode } from "react";

import { Icon } from "@/components/ui/icon";
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
            <PopoverPrimitive.Root>
              <PopoverPrimitive.Trigger asChild>
                <button
                  type="button"
                  className={styles.help}
                  aria-label={helpLabel ?? `More about ${title}`}
                >
                  <Icon name="info" size="sm" />
                </button>
              </PopoverPrimitive.Trigger>
              <PopoverPrimitive.Portal>
                <PopoverPrimitive.Content
                  sideOffset={8}
                  collisionPadding={8}
                  className={styles.helpContent}
                >
                  {help}
                  <PopoverPrimitive.Arrow className={styles.helpArrow} />
                </PopoverPrimitive.Content>
              </PopoverPrimitive.Portal>
            </PopoverPrimitive.Root>
          ) : null}
          {badge}
        </span>
        {note}
      </div>
      <div className={styles.control} data-settings-control>{children}</div>
    </div>
  );
}
