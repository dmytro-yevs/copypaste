/** A settings pane long enough to scroll needs somewhere for the eye to stop:
 *  the Service tab's nine rows in one undifferentiated list meant a user
 *  looking for one of them read all nine. */
import type { ReactNode } from "react";

interface SectionProps {
  title: string;
  description?: string;
  children: ReactNode;
}

export function Section({ title, description, children }: SectionProps) {
  return (
    <section className="flex flex-col">
      <h2 className="pt-s-2 text-sm font-semibold">{title}</h2>
      {description !== undefined && (
        <p className="py-s-1 text-xs text-muted-foreground">{description}</p>
      )}
      {children}
    </section>
  );
}
