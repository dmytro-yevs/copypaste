import { Icon, type IconName } from "@/components/ui";
import { cn } from "@/lib/cn";
import styles from "./DeviceMeta.module.css";

export type DeviceMetaKind = "desktop" | "laptop" | "phone" | "tablet" | "unknown";

function canonicalKind(label: string, kind: DeviceMetaKind | undefined): DeviceMetaKind {
  if (kind && kind !== "unknown") return kind;
  const normalized = label.trim().toLowerCase();
  if (normalized === "mac" || normalized.includes("windows")) return "desktop";
  if (normalized.includes("iphone") || normalized.includes("android") || normalized.includes("phone")) return "phone";
  if (normalized.includes("ipad") || normalized.includes("tablet")) return "tablet";
  return "unknown";
}

const ICONS: Record<DeviceMetaKind, IconName> = {
  desktop: "monitor",
  laptop: "monitor",
  phone: "mobile",
  tablet: "tablet",
  unknown: "devices",
};

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
  const resolved = canonicalKind(label, kind);
  return (
    <span className={cn(styles.root, className)} title={label} data-device-kind={resolved}>
      <Icon name={ICONS[resolved]} size="xs" weight="regular" />
      {iconOnly ? null : <span className={styles.label}>{label}</span>}
    </span>
  );
}
