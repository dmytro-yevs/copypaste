import { Icon, type IconName } from "@/components/ui";
import type { DeviceStatusPresentation } from "@/features/devices/model/devicePresentation";
import { cn } from "@/lib/cn";
import styles from "./DeviceStatus.module.css";

const STATUS_ICON: Record<DeviceStatusPresentation["tone"], IconName> = {
  ready: "checkCircle",
  busy: "refresh",
  attention: "alert",
  danger: "xCircle",
  neutral: "circle",
};

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
      className={cn(styles.root, className)}
    >
      <Icon
        name={STATUS_ICON[status.tone]}
        size="xs"
        className={styles.icon}
        aria-hidden="true"
      />
      <span className={styles.label}>{status.label}</span>
    </span>
  );
}
