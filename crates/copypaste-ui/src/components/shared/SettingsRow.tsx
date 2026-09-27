/**
 * A11Y-15: it wraps rather than truncating, so at the 720px minimum the control
 * drops below its label instead of being pushed off the pane.
 *
 * `badge` and `note` are separate slots: a badge is a property of the *field*,
 * true whether or not it has been touched, and sits beside the title; a note is
 * the field's state right now and sits under the description. Neither is a
 * footnote at the bottom of the pane, which is read only after the user has
 * wondered why nothing happened.
 */
import * as PopoverPrimitive from "@radix-ui/react-popover";
import type { ReactNode } from "react";

import { Icon } from "@/components/ui/icon";
import styles from "./SettingsRow.module.css";

interface SettingsRowProps {
  title: string;
  description?: string;
  descriptionId?: string;
  help?: ReactNode;
  helpLabel?: string;
  badge?: ReactNode;
  note?: ReactNode;
  children: ReactNode;
}

export function SettingsRow({
  title,
  description,
  descriptionId,
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
        {description && (
          <span id={descriptionId} className={styles.description}>
            {description}
          </span>
        )}
        {note}
      </div>
      <div className={styles.control} data-settings-control>{children}</div>
    </div>
  );
}
