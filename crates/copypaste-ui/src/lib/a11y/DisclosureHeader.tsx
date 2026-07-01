import { type ReactNode } from "react";

export interface DisclosureHeaderProps {
  /** Whether the controlled region is expanded. */
  expanded: boolean;
  /** id of the region this header controls (wired to `aria-controls`). */
  controls: string;
  /** Toggle the expanded state. */
  onToggle: () => void;
  className?: string;
  "aria-label"?: string;
  children: ReactNode;
}

/**
 * Typed disclosure-header primitive (design.md Decision 3 / task 2.10). A button
 * that toggles an expandable region, exposing `aria-expanded`/`aria-controls`
 * (React state is the source of truth; ARIA is derived from it). It is NOT a
 * `.btn` — it's its own allowed button-shaped primitive, so callers style it via
 * their own class (e.g. `.devrow__head` in slice 4), never the `.btn` family.
 */
export function DisclosureHeader({
  expanded,
  controls,
  onToggle,
  className,
  "aria-label": ariaLabel,
  children,
}: DisclosureHeaderProps) {
  return (
    <button
      type="button"
      className={className}
      aria-expanded={expanded}
      aria-controls={controls}
      aria-label={ariaLabel}
      onClick={onToggle}
    >
      {children}
    </button>
  );
}
