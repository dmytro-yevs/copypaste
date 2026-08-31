import { Icon, type IconName } from "@/components/ui";
import { cn } from "@/lib/cn";
import { deviceClassDescriptor } from "@/lib/deviceIdentity";
import type { DeviceClass } from "@/lib/ipc";
import styles from "./DeviceMeta.module.css";

export type DeviceMetaKind = DeviceClass;

export function DeviceMeta({
  label,
  kind,
  iconOnly = false,
  className,
}: {
  label: string;
  kind?: DeviceMetaKind;
  iconOnly?: boolean;
  className?: string;
}) {
  const resolved = kind ?? "unknown";
  const icon: IconName = deviceClassDescriptor(resolved).icon;
  return (
    <span className={cn(styles.root, className)} title={label} data-device-kind={resolved}>
      <Icon name={icon} size="xs" weight="regular" />
      {iconOnly ? null : <span className={styles.label}>{label}</span>}
    </span>
  );
}
