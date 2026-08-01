import type { ComponentProps } from "react";
import * as TabsPrimitive from "@radix-ui/react-tabs";

import { cn } from "@/lib/cn";

function Tabs({ className, ...props }: ComponentProps<typeof TabsPrimitive.Root>) {
  return (
    <TabsPrimitive.Root
      data-slot="tabs"
      className={cn("flex flex-col gap-2", className)}
      {...props}
    />
  );
}

/**
 * A11Y-15: the tab row **wraps** rather than scrolling. At the 720px minimum
 * window width v1 hid its last tab behind a scrollbar-less scroller
 * (CopyPaste-g27b.31), so `flex-wrap` here is a requirement, not a preference —
 * `shell-reflow.test.tsx` asserts it.
 */
type TabsListProps = ComponentProps<typeof TabsPrimitive.List> & {
  variant?: "bare" | "floating";
  equalWidth?: boolean;
};

function TabsList({
  className,
  variant = "floating",
  equalWidth = false,
  ...props
}: TabsListProps) {
  return (
    <TabsPrimitive.List
      data-slot="tabs-list"
      data-equal-width={equalWidth || undefined}
      className={cn(
        variant === "floating"
          ? equalWidth
            ? "grid w-full grid-flow-col auto-cols-fr items-center gap-1 rounded-full border border-border bg-panel p-1 text-muted-foreground shadow-sm backdrop-blur-sm"
            : "inline-flex w-fit flex-wrap items-center justify-start gap-1 rounded-full border border-border bg-panel p-1 text-muted-foreground shadow-sm backdrop-blur-sm"
          : equalWidth
            ? "grid w-full grid-flow-col auto-cols-fr"
            : "flex",
        className,
      )}
      {...props}
    />
  );
}

function TabsTrigger({
  className,
  ...props
}: ComponentProps<typeof TabsPrimitive.Trigger>) {
  return (
    <TabsPrimitive.Trigger
      data-slot="tabs-trigger"
      className={cn(
        "inline-flex min-h-[var(--tap-min)] min-w-[var(--tap-min)] flex-1 items-center justify-center gap-1.5 rounded-full border border-transparent px-2 py-1 text-sm font-medium whitespace-nowrap transition-[color,background-color,box-shadow] hover:bg-accent hover:text-accent-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring focus-visible:outline-1 disabled:pointer-events-none disabled:opacity-50",
        "data-[state=active]:bg-muted data-[state=active]:text-foreground data-[state=active]:shadow-xs",
        "[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        className,
      )}
      {...props}
    />
  );
}

function TabsContent({
  className,
  ...props
}: ComponentProps<typeof TabsPrimitive.Content>) {
  return (
    <TabsPrimitive.Content
      data-slot="tabs-content"
      className={cn("flex-1 outline-none", className)}
      {...props}
    />
  );
}

export { Tabs, TabsList, TabsTrigger, TabsContent };
