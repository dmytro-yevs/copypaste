import * as PopoverPrimitive from "@radix-ui/react-popover";
import type { ReactNode } from "react";

import { Button } from "@/components/ui/button";
import styles from "./HelpPopover.module.css";

export function HelpPopover({ content, label }: { content: ReactNode; label: string }) {
  return (
    <PopoverPrimitive.Root>
      <PopoverPrimitive.Trigger asChild>
        <Button variant="ghost" size="compactIcon" className={styles.trigger} label={label} icon="info" />
      </PopoverPrimitive.Trigger>
      <PopoverPrimitive.Portal>
        <PopoverPrimitive.Content sideOffset={8} collisionPadding={8} className={styles.content}>
          {content}
          <PopoverPrimitive.Arrow className={styles.arrow} />
        </PopoverPrimitive.Content>
      </PopoverPrimitive.Portal>
    </PopoverPrimitive.Root>
  );
}
