import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import type { ComponentProps, ReactNode } from "react";

import { cn } from "@/lib/cn";
import styles from "./tooltip.module.css";

export function Tooltip({
  children,
  content,
  sideOffset = 7,
}: {
  children: ReactNode;
  content: ReactNode;
  sideOffset?: number;
}) {
  return (
    <TooltipPrimitive.Root disableHoverableContent>
      <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
      <TooltipPrimitive.Portal>
        <TooltipPrimitive.Content sideOffset={sideOffset} className={styles.content}>
          {content}
        </TooltipPrimitive.Content>
      </TooltipPrimitive.Portal>
    </TooltipPrimitive.Root>
  );
}

export function TooltipContent({ className, ...props }: ComponentProps<typeof TooltipPrimitive.Content>) {
  return <TooltipPrimitive.Content className={cn(styles.content, className)} {...props} />;
}

export const TooltipProvider = TooltipPrimitive.Provider;
export const TooltipRoot = TooltipPrimitive.Root;
export const TooltipTrigger = TooltipPrimitive.Trigger;
export const TooltipPortal = TooltipPrimitive.Portal;
