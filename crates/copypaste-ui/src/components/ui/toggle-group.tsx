import { type ComponentProps, createContext, useContext } from "react";
import * as ToggleGroupPrimitive from "@radix-ui/react-toggle-group";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";

const toggleVariants = cva(
  "inline-flex min-h-[var(--tap-min)] min-w-[var(--tap-min)] items-center justify-center gap-2 rounded-full text-sm font-medium whitespace-nowrap transition-[background-color,color,box-shadow] outline-none hover:bg-muted hover:text-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 data-[state=on]:bg-muted data-[state=on]:text-foreground data-[state=on]:shadow-xs [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      size: {
        default: "h-9 min-w-[var(--tap-min)] px-2",
        sm: "h-8 min-w-[var(--tap-min)] px-1.5",
        lg: "h-10 min-w-[var(--tap-min)] px-2.5",
      },
    },
    defaultVariants: { size: "default" },
  },
);

type ToggleGroupContextValue = VariantProps<typeof toggleVariants> & {
  equalWidth: boolean;
};

const ToggleGroupContext = createContext<ToggleGroupContextValue>({
  size: "default",
  equalWidth: false,
});

function ToggleGroup({
  className,
  size,
  equalWidth = false,
  children,
  ...props
}: ComponentProps<typeof ToggleGroupPrimitive.Root> &
  VariantProps<typeof toggleVariants> & { equalWidth?: boolean }) {
  return (
    <ToggleGroupPrimitive.Root
      data-slot="toggle-group"
      data-equal-width={equalWidth || undefined}
      className={cn(
        equalWidth
          ? "grid w-fit auto-cols-fr grid-flow-col items-center gap-1 rounded-full border border-border bg-panel/90 p-1 shadow-sm backdrop-blur-sm"
          : "flex w-fit flex-wrap items-center gap-1 rounded-full border border-border bg-panel/90 p-1 shadow-sm backdrop-blur-sm",
        className,
      )}
      {...props}
    >
      <ToggleGroupContext.Provider value={{ size, equalWidth }}>
        {children}
      </ToggleGroupContext.Provider>
    </ToggleGroupPrimitive.Root>
  );
}

function ToggleGroupItem({
  className,
  children,
  size,
  ...props
}: ComponentProps<typeof ToggleGroupPrimitive.Item> &
  VariantProps<typeof toggleVariants>) {
  const context = useContext(ToggleGroupContext);
  return (
    <ToggleGroupPrimitive.Item
      data-slot="toggle-group-item"
      className={cn(
        toggleVariants({ size: context.size ?? size }),
        context.equalWidth && "min-w-0 w-full",
        className,
      )}
      {...props}
    >
      {children}
    </ToggleGroupPrimitive.Item>
  );
}

export { ToggleGroup, ToggleGroupItem, toggleVariants };
