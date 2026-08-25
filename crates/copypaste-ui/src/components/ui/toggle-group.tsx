import { type ComponentProps, createContext, useContext } from "react";
import * as ToggleGroupPrimitive from "@radix-ui/react-toggle-group";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import styles from "./toggle-group.module.css";

const toggleVariants = cva(
  styles.item,
  {
    variants: {
      size: {
        default: styles.md,
        sm: styles.sm,
        lg: styles.lg,
      },
    },
    defaultVariants: { size: "default" },
  },
);

const toggleGroupVariants = cva(styles.group, {
  variants: {
    equalWidth: {
      true: styles.equalWidth,
      false: styles.wrapping,
    },
  },
  defaultVariants: { equalWidth: false },
});

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
    <div className={styles.frame} data-equal-width={equalWidth || undefined}>
      <ToggleGroupPrimitive.Root
        data-slot="toggle-group"
        data-equal-width={equalWidth || undefined}
        className={cn(toggleGroupVariants({ equalWidth }), className)}
        {...props}
      >
        <ToggleGroupContext.Provider value={{ size, equalWidth }}>
          {children}
        </ToggleGroupContext.Provider>
      </ToggleGroupPrimitive.Root>
    </div>
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
        context.equalWidth && styles.fill,
        className,
      )}
      {...props}
    >
      <span className={styles.itemContent}>{children}</span>
    </ToggleGroupPrimitive.Item>
  );
}

export { ToggleGroup, ToggleGroupItem, toggleVariants };
