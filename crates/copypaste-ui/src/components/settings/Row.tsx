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
import type { ReactNode } from "react";

interface RowProps {
  title: string;
  description?: string;
  descriptionId?: string;
  badge?: ReactNode;
  note?: ReactNode;
  children: ReactNode;
}

export function Row({
  title,
  description,
  descriptionId,
  badge,
  note,
  children,
}: RowProps) {
  return (
    <div
      data-settings-search-target={`row:${title}`}
      className="relative flex flex-wrap items-start justify-between gap-s-3 px-s-3 py-s-3 last:after:hidden after:absolute after:right-s-3 after:bottom-0 after:left-s-3 after:h-px after:bg-divider"
    >
      <div className="flex min-w-0 max-w-[380px] flex-1 flex-col gap-s-1 sm:min-w-[200px]">
        <span className="flex flex-wrap items-center gap-s-2 text-sm font-medium">
          {title}
          {badge}
        </span>
        {description && (
          <span id={descriptionId} className="text-xs text-muted-foreground">
            {description}
          </span>
        )}
        {note}
      </div>
      <div className="flex min-w-0 shrink-0 items-center max-sm:w-full max-sm:justify-end">{children}</div>
    </div>
  );
}
