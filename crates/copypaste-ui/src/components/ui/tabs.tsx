import type { ComponentProps } from "react";
import * as TabsPrimitive from "@radix-ui/react-tabs";
import { cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import styles from "./tabs.module.css";

const tabsListVariants = cva(undefined, {
  variants: {
    variant: {
      bare: undefined,
      floating: styles.floating,
    },
    equalWidth: {
      true: styles.equalWidth,
      false: undefined,
    },
  },
  compoundVariants: [
    { variant: "bare", equalWidth: false, className: styles.bareNatural },
    { variant: "floating", equalWidth: false, className: styles.floatingNatural },
    { variant: "floating", equalWidth: true, className: styles.floatingEqual },
  ],
  defaultVariants: {
    variant: "floating",
    equalWidth: false,
  },
});

function Tabs({ className, ...props }: ComponentProps<typeof TabsPrimitive.Root>) {
  return (
    <TabsPrimitive.Root
      data-slot="tabs"
      className={cn(styles.root, className)}
      {...props}
    />
  );
}

/**
 * A11Y-15: the tab row **wraps** rather than scrolling. At the 720px minimum
 * window width must not hide its last tab behind a scrollbar-less scroller
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
  const list = (
    <TabsPrimitive.List
      data-slot="tabs-list"
      data-equal-width={equalWidth || undefined}
      className={cn(tabsListVariants({ variant, equalWidth }), className)}
      {...props}
    />
  );
  return variant === "floating" ? (
    <div
      className={styles.floatingFrame}
      data-equal-width={equalWidth || undefined}
    >
      {list}
    </div>
  ) : list;
}

function TabsTrigger({
  className,
  children,
  ...props
}: ComponentProps<typeof TabsPrimitive.Trigger>) {
  return (
    <TabsPrimitive.Trigger
      data-slot="tabs-trigger"
      className={cn(styles.trigger, className)}
      {...props}
    >
      <span className={styles.triggerContent}>{children}</span>
    </TabsPrimitive.Trigger>
  );
}

function TabsContent({
  className,
  ...props
}: ComponentProps<typeof TabsPrimitive.Content>) {
  return (
    <TabsPrimitive.Content
      data-slot="tabs-content"
      className={cn(styles.content, className)}
      {...props}
    />
  );
}

export { Tabs, TabsList, TabsTrigger, TabsContent };
