import { Button, Icon } from "@/components/ui";
import { DeviceKindIcon } from "@/features/devices/components/DeviceKindIcon";
import { DeviceStatus } from "@/features/devices/components/DeviceStatus";
import type {
  DevicePresentationIdentity,
  DeviceStatusPresentation,
} from "@/features/devices/model/devicePresentation";
import { cn } from "@/lib/cn";
import styles from "./DeviceCard.module.css";

interface DeviceCardProps {
  name: string;
  identity: DevicePresentationIdentity;
  trustLabel: string;
  status: DeviceStatusPresentation;
  selectionKey: string;
  selected: boolean;
  onSelect: () => void;
  className?: string;
}

export function DeviceCard({
  name,
  identity,
  trustLabel,
  status,
  selectionKey,
  selected,
  onSelect,
  className,
}: DeviceCardProps) {
  return (
      <Button
        type="button"
        variant="ghost"
        size="md"
        aria-expanded={selected}
        aria-busy={status.busy || undefined}
        aria-label={`${name}. ${trustLabel}. ${status.label}.`}
        className={cn(styles.card, className)}
        data-status={status.tone}
        data-selected={selected || undefined}
        data-device-selection-key={selectionKey}
        onClick={onSelect}
      >
        <span className={styles.identityWell} aria-hidden="true">
          <DeviceKindIcon identity={identity} />
        </span>
          <span className={styles.copy}>
          <span className={styles.name}>{name}</span>
          <span className={styles.detailLine}>
            <span className={styles.detailLayout}>
              <span className={styles.meta}>{trustLabel}</span>
              <span className={styles.separator} aria-hidden="true">·</span>
              <DeviceStatus status={status} className={styles.status} />
            </span>
          </span>
        </span>
        <Icon name="caretRight" size="sm" className={styles.chevron} />
      </Button>
  );
}
