/**
 * A11Y-15: it wraps rather than truncating, so at the 720px minimum the control
 * drops below its label instead of being pushed off the pane.
 *
 * `badge` and `note` are separate slots: a badge is a property of the *field* —
 * "this one needs a restart" is true whether or not it has been touched — and
 * sits beside the title; a note is the field's state right now and sits under
 * the description. Neither is a footnote at the bottom of the pane, which is
 * read only after the user has wondered why nothing happened.
 */
import type { ReactNode } from "react";

interface RowProps {
  title: string;
  description?: string;
  badge?: ReactNode;
  note?: ReactNode;
  children: ReactNode;
}

export function Row({ title, description, badge, note, children }: RowProps) {
  return (
    <div className="flex flex-wrap items-start justify-between gap-s-3 border-b border-divider py-s-3 last:border-b-0">
      <div className="flex min-w-[200px] max-w-[380px] flex-1 flex-col gap-s-1">
        <span className="flex flex-wrap items-center gap-s-2 text-sm font-medium">
          {title}
          {badge}
        </span>
        {description && (
          <span className="text-xs text-muted-foreground">{description}</span>
        )}
        {note}
      </div>
      <div className="flex shrink-0 items-center">{children}</div>
    </div>
  );
}
