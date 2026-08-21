import type { ComponentProps } from "react";
import { ChevronLeft } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/cn";

function PaneHeader({ className, ...props }: ComponentProps<"header">) {
  return (
    <header
      className={cn(
        "chrome flex w-full shrink-0 items-center gap-s-2 border-b border-divider px-s-3 py-s-2 sm:px-s-4",
        className,
      )}
      {...props}
    />
  );
}

function PaneHeaderBackButton({
  className,
  label,
  ...props
}: Omit<ComponentProps<typeof Button>, "children" | "size" | "variant"> & {
  label: string;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      aria-label={label}
      title={label}
      className={className}
      {...props}
    >
      <ChevronLeft aria-hidden="true" />
    </Button>
  );
}

function PaneHeaderTitle({ className, ...props }: ComponentProps<"h1">) {
  return <h1 className={cn("min-w-0 flex-1 text-sm font-semibold", className)} {...props} />;
}

function PaneHeaderActions({ className, ...props }: ComponentProps<"div">) {
  return <div className={cn("flex min-w-0 items-center gap-s-2", className)} {...props} />;
}

export { PaneHeader, PaneHeaderActions, PaneHeaderBackButton, PaneHeaderTitle };
