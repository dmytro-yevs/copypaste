import * as PopoverPrimitive from "@radix-ui/react-popover";
import type { ReactNode } from "react";

import { Icon } from "@/components/ui/icon";
import styles from "./HelpPopover.module.css";

export function HelpPopover({ content, label }: { content: ReactNode; label: string }) {
  return (
    <PopoverPrimitive.Root>
      <PopoverPrimitive.Trigger asChild>
        <button type="button" className={styles.trigger} aria-label={label}>
          <Icon name="info" size="sm" />
        </button>
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
