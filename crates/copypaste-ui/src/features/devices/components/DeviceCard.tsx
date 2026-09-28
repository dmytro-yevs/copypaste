import type { ReactNode } from "react";

import { StateView } from "@/components/shared/StateView";
import { Button, Icon } from "@/components/ui";
import { deviceIconKind, deviceStatusMode } from "@/features/devices/model/devicePresentation";
import type {
  DevicePresentationIdentity,
  DeviceStatusPresentation,
} from "@/features/devices/model/devicePresentation";
import { cn } from "@/lib/cn";
import styles from "./DeviceCard.module.css";

interface DeviceCardBaseProps {
  name: string;
  identity: DevicePresentationIdentity;
  selectionKey: string;
  selected: boolean;
  onSelect: () => void;
  ariaLabel: string;
  className?: string;
}

type DeviceCardProps = DeviceCardBaseProps & (
  | {
      appearance?: "paired";
      trustLabel: string;
      status: DeviceStatusPresentation;
      detail?: never;
    }
  | {
      appearance: "discovery";
      detail: ReactNode;
      trustLabel?: never;
      status?: never;
    }
);

/** Shared selectable card frame for paired and discovered devices. */
export function DeviceCard({
  name,
  identity,
  detail,
  trustLabel,
  selectionKey,
  selected,
  onSelect,
  ariaLabel,
  status,
  appearance = "paired",
  className,
}: DeviceCardProps) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="md"
      aria-expanded={selected}
      aria-busy={status?.busy || undefined}
      aria-label={ariaLabel}
      className={cn(styles.card, className)}
      data-appearance={appearance}
      data-status={status?.tone}
      data-selected={selected || undefined}
      data-device-selection-key={selectionKey}
      onClick={onSelect}
    >
      <span className={styles.identityWell} aria-hidden="true">
        <Icon name={deviceIconKind(identity)} size="md" />
      </span>
      <span className={styles.copy}>
        <span className={styles.name}>{name}</span>
        <span className={styles.detailLine}>
          {appearance === "paired" && status && trustLabel ? (
            <span className={styles.detailLayout}>
              <span className={styles.meta}>{trustLabel}</span>
              <span className={styles.separator} aria-hidden="true">·</span>
              <StateView
                mode={deviceStatusMode(status)}
                placement="control"
                title={status.label}
                icon={status.icon}
                role={status.a11y.role ?? "presentation"}
                aria-live={status.a11y.live}
                aria-busy={status.busy || undefined}
                className={styles.status}
              />
            </span>
          ) : detail}
        </span>
      </span>
      <Icon name="caretRight" size="sm" className={styles.chevron} />
    </Button>
  );
}
