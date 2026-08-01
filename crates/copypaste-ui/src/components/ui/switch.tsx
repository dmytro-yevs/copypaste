import type { ComponentProps } from "react";
import * as SwitchPrimitive from "@radix-ui/react-switch";

import { cn } from "@/lib/cn";

function Switch({
  className,
  ...props
}: ComponentProps<typeof SwitchPrimitive.Root>) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        "peer group inline-flex h-[var(--tap-min)] w-12 shrink-0 items-center outline-none focus-visible:ring-[3px] focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      {...props}
    >
      <span
        data-slot="switch-track"
        className="flex h-7 w-12 items-center rounded-full border border-border-strong bg-raised-2 p-[3px] shadow-xs transition-[background-color,border-color] group-data-[state=checked]:border-primary group-data-[state=checked]:bg-primary"
      >
        <SwitchPrimitive.Thumb
          data-slot="switch-thumb"
          className="pointer-events-none block size-5 rounded-full bg-card shadow-xs transition-transform group-data-[state=checked]:translate-x-5 group-data-[state=checked]:bg-primary-foreground"
        />
      </span>
    </SwitchPrimitive.Root>
  );
}

export { Switch };
