import type { ComponentProps } from "react";
import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { Check } from "lucide-react";

import { cn } from "@/lib/cn";

function Checkbox({
  className,
  ...props
}: ComponentProps<typeof CheckboxPrimitive.Root>) {
  return (
    <CheckboxPrimitive.Root
      data-slot="checkbox"
      className={cn(
        "peer group inline-flex size-[var(--tap-min)] shrink-0 items-center justify-center outline-none focus-visible:ring-[3px] focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      {...props}
    >
      <span
        data-slot="checkbox-control"
        className="flex size-4 items-center justify-center rounded-[4px] border border-border-strong shadow-xs transition-shadow group-data-[state=checked]:border-transparent group-data-[state=checked]:bg-primary group-data-[state=checked]:text-primary-foreground"
      >
        <CheckboxPrimitive.Indicator
          data-slot="checkbox-indicator"
          className="flex size-full items-center justify-center text-current"
        >
          <Check className="size-3.5" strokeWidth={3} aria-hidden="true" />
        </CheckboxPrimitive.Indicator>
      </span>
    </CheckboxPrimitive.Root>
  );
}

export { Checkbox };
