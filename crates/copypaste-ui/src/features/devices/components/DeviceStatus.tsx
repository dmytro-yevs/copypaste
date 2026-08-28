import { Icon } from "@/components/ui";
import type { DeviceStatusPresentation } from "@/features/devices/model/devicePresentation";
import { cn } from "@/lib/cn";
import styles from "./DeviceStatus.module.css";

export function DeviceStatus({
  status,
  className,
}: {
  status: DeviceStatusPresentation;
  className?: string;
}) {
  return (
    <span
      data-slot="device-status"
      data-tone={status.tone}
      aria-busy={status.busy || undefined}
      role={status.a11y.role}
      aria-live={status.a11y.live}
      className={cn(styles.root, className)}
    >
      <Icon
        name={status.icon}
        size="xs"
        className={styles.icon}
        aria-hidden="true"
      />
      <span className={styles.label}>{status.label}</span>
    </span>
  );
}
