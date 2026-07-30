/**
 * One setting: a title, a sentence of explanation, and the control.
 *
 * A11Y-15: it wraps rather than truncating, so at the 720px minimum the control
 * drops below its label instead of being pushed off the pane.
 */
import type { ReactNode } from "react";

interface RowProps {
  title: string;
  description?: string;
  children: ReactNode;
}

export function Row({ title, description, children }: RowProps) {
  return (
    <div className="flex flex-wrap items-start justify-between gap-s-3 border-b border-divider py-s-3 last:border-b-0">
      <div className="flex min-w-[200px] max-w-[380px] flex-1 flex-col gap-s-1">
        <span className="text-sm font-medium">{title}</span>
        {description && (
          <span className="text-xs text-muted-foreground">{description}</span>
        )}
      </div>
      <div className="flex shrink-0 items-center">{children}</div>
    </div>
  );
}
