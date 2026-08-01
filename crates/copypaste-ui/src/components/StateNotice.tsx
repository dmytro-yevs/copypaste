import type { LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";

type StateNoticeTone = "neutral" | "info" | "attention" | "danger";

interface StateNoticeProps {
  icon: LucideIcon;
  title: string;
  body?: string;
  tone?: StateNoticeTone;
  busy?: boolean;
  action?: {
    label: string;
    onClick: () => void;
    icon?: LucideIcon;
    disabled?: boolean;
  };
}

export function StateNotice({
  icon: Icon,
  title,
  body,
  tone = "neutral",
  busy = false,
  action,
}: StateNoticeProps) {
  const ActionIcon = action?.icon;
  const toneClasses = {
    neutral: "bg-muted text-muted-foreground",
    info: "bg-info/15 text-info-strong",
    attention: "bg-warn/15 text-warn-strong",
    danger: "bg-err/15 text-err-strong",
  }[tone];

  return (
    <section
      data-slot="state-notice"
      role={tone === "danger" ? "alert" : busy ? "status" : undefined}
      aria-busy={busy || undefined}
      className="flex w-full flex-wrap items-center gap-s-3 rounded-xl border border-border bg-card p-s-3 shadow-sm"
    >
      <span
        aria-hidden="true"
        className={`flex size-[var(--sz-tile)] shrink-0 items-center justify-center rounded-lg ${toneClasses}`}
      >
        <Icon className={busy ? "animate-spin motion-reduce:animate-none" : undefined} size={18} />
      </span>
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-foreground">{title}</p>
        {body && <p className="mt-0.5 text-xs text-muted-foreground">{body}</p>}
      </div>
      {action && (
        <Button
          variant="outline"
          size="sm"
          disabled={action.disabled}
          onClick={action.onClick}
        >
          {ActionIcon && <ActionIcon aria-hidden="true" />}
          {action.label}
        </Button>
      )}
    </section>
  );
}
