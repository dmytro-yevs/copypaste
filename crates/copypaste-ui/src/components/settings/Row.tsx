/**
 * One setting: a title, a sentence of explanation, and the control.
 *
 * A11Y-15: it wraps rather than truncating, so at the 720px minimum the control
 * drops below its label instead of being pushed off the pane.
 *
 * `badge` and `note` are separate slots because they say different kinds of
 * thing. A badge is a property of the *field* — "this one needs a restart" is
 * true whether or not it has been touched — and belongs beside the title where
 * it is read before the control is used. A note is the field's state right now,
 * and belongs under the description where it can be a whole sentence. Neither
 * is a footnote: a liveness rule at the bottom of a pane is a rule nobody reads
 * until after they have wondered why nothing happened.
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
