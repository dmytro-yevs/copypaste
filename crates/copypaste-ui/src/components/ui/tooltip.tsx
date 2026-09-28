import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import type { ReactNode } from "react";

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

export const TooltipProvider = TooltipPrimitive.Provider;
