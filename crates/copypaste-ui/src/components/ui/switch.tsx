import type { ComponentProps } from "react";
import * as SwitchPrimitive from "@radix-ui/react-switch";

import { cn } from "@/lib/cn";
import styles from "./switch.module.css";

function Switch({
  className,
  ...props
}: ComponentProps<typeof SwitchPrimitive.Root>) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(styles.root, className)}
      {...props}
    >
      <span
        data-slot="switch-track"
        className={styles.track}
      >
        <SwitchPrimitive.Thumb
          data-slot="switch-thumb"
          className={styles.thumb}
        />
      </span>
    </SwitchPrimitive.Root>
  );
}

export { Switch };
