import type { ComponentProps } from "react";
import { ChevronDown } from "lucide-react";

import { cn } from "@/lib/cn";

function NativeSelect({ className, ...props }: ComponentProps<"select">) {
  return (
    <span className="relative inline-flex shrink-0">
      <select
        data-slot="native-select"
        className={cn(
          "h-9 min-h-[var(--tap-min)] appearance-none rounded-md border border-input bg-panel px-3 pr-10 text-sm text-foreground outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50",
          className,
        )}
        {...props}
      />
      <ChevronDown
        aria-hidden="true"
        data-testid="native-select-chevron"
        className="pointer-events-none absolute right-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
      />
    </span>
  );
}

export { NativeSelect };
