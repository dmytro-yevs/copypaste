import type { LucideIcon } from "lucide-react";

import { cn } from "@/lib/cn";

export interface StepperItem {
  id: string;
  label: string;
  stateLabel: string;
  icon: LucideIcon;
  done?: boolean;
  current?: boolean;
}

function Stepper({
  label,
  items,
  className,
}: {
  label: string;
  items: readonly StepperItem[];
  className?: string;
}) {
  return (
    <ol aria-label={label} className={cn("flex flex-col gap-s-1", className)}>
      {items.map((item) => {
        const Icon = item.icon;
        return (
          <li
            key={item.id}
            data-step={item.id}
            className="flex flex-wrap items-center gap-s-2 border-b border-divider py-s-2 text-sm last:border-b-0"
          >
            <Icon
              size={15}
              aria-hidden="true"
              className={cn(
                "shrink-0",
                item.done ? "text-ok-strong" : "text-muted-foreground",
              )}
            />
            <span className={cn("min-w-0 flex-1", item.current && "font-medium")}>
              {item.label}
            </span>
            <span className="shrink-0 text-xs text-muted-foreground">{item.stateLabel}</span>
          </li>
        );
      })}
    </ol>
  );
}

export { Stepper };
