/**
 * Loading / empty / offline / error placeholder.
 *
 * Every string it can be given comes from manifest 06 §3.1.11; none of them is
 * ever derived from an error object (INV-12). The spinner and the icon are
 * deliberately real, visible elements: v1 shipped classless empty `<span>`s
 * that rendered as nothing and were indistinguishable from a layout bug
 * (CopyPaste-8ebg.29).
 */
import type { LucideIcon } from "lucide-react";
import { LoaderCircle } from "lucide-react";

interface EmptyStateProps {
  icon?: LucideIcon;
  busy?: boolean;
  title: string;
  body: string;
  action?: { label: string; onClick: () => void };
}

export function EmptyState({
  icon: Icon,
  busy = false,
  title,
  body,
  action,
}: EmptyStateProps) {
  return (
    <div
      className="flex min-h-0 flex-1 flex-col items-center justify-center gap-s-4 px-[var(--pad-empty)] text-center"
      aria-busy={busy || undefined}
    >
      <span
        aria-hidden="true"
        className="flex size-[44px] items-center justify-center rounded-empty-ic bg-elevated text-dim"
      >
        {busy ? (
          <LoaderCircle size={20} className="animate-spin" />
        ) : Icon ? (
          <Icon size={20} />
        ) : null}
      </span>

      <div className="flex flex-col gap-s-2">
        <p className="text-fs-lg font-medium text-text">{title}</p>
        <p className="max-w-[var(--content-max-width)] text-fs-md text-dim">
          {body}
        </p>
      </div>

      {action && (
        <button
          type="button"
          onClick={action.onClick}
          className="rounded-ctl border border-border bg-raised px-[13px] py-[7px] text-fs-md text-text transition-colors duration-[var(--dur-fast)] hover:bg-raised-2"
        >
          {action.label}
        </button>
      )}
    </div>
  );
}
