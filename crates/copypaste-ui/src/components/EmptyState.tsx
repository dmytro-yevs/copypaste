/**
 * The spinner and the icon are real, visible elements: v1 shipped classless
 * empty `<span>`s that rendered as nothing and were indistinguishable from a
 * layout bug (CopyPaste-8ebg.29, bdac.2). Loading copy is static text — a
 * pulsing one ignored reduced motion (MOT-21).
 */
import type { LucideIcon } from "lucide-react";
import { LoaderCircle } from "lucide-react";
import type { ReactNode } from "react";

import { Button } from "@/components/ui/button";

interface EmptyStateProps {
  icon?: LucideIcon;
  busy?: boolean;
  title: string;
  body: string;
  action?: { label: string; onClick: () => void; icon?: LucideIcon };
  secondary?: ReactNode;
}

export function EmptyState({
  icon: Icon,
  busy = false,
  title,
  body,
  action,
  secondary,
}: EmptyStateProps) {
  const ActionIcon = action?.icon;

  return (
    <div
      className="flex min-h-0 flex-1 flex-col items-center justify-center gap-s-3 px-[var(--pad-empty)] py-s-6 text-center"
      aria-busy={busy || undefined}
    >
      <span
        aria-hidden="true"
        className="flex size-[44px] items-center justify-center rounded-empty-ic bg-muted text-muted-foreground"
      >
        {busy ? (
          <LoaderCircle size={20} className="animate-spin" />
        ) : Icon ? (
          <Icon size={20} />
        ) : null}
      </span>

      <div className="flex flex-col gap-s-1">
        <p className="text-lg font-medium text-foreground">{title}</p>
        <p className="max-w-[var(--content-max-width)] text-sm text-muted-foreground">
          {body}
        </p>
      </div>

      {action && (
        <Button onClick={action.onClick}>
          {ActionIcon && <ActionIcon aria-hidden="true" />}
          {action.label}
        </Button>
      )}

      {secondary}
    </div>
  );
}
