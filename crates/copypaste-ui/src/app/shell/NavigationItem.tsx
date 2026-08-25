import type { ComponentProps } from "react";

import { Button, Icon, Tooltip, type IconName } from "@/components/ui";
import styles from "./NavigationItem.module.css";

export function NavigationItem({ icon, label, layout, active, ...props }: Omit<ComponentProps<typeof Button>, "children" | "className" | "size" | "variant"> & {
  icon: IconName;
  label: string;
  layout: "sidebar" | "dock";
  active: boolean;
}) {
  const button = (
    <Button {...props} type="button" variant="ghost" size="md" className={styles.item} data-layout={layout} aria-label={label} aria-current={active ? "page" : undefined}>
      <Icon name={icon} size="sm" weight="regular" />
      <span>{label}</span>
    </Button>
  );
  return layout === "sidebar" ? (
    <Tooltip content={label}>{button}</Tooltip>
  ) : button;
}
