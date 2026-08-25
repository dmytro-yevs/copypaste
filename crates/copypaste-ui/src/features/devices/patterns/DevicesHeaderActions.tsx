import { Icon } from "@/components/ui/icon";
import type { RefObject } from "react";

import { Button } from "@/components/ui";
import styles from "./DevicesHeaderActions.module.css";

interface DevicesHeaderActionsProps {
  pairButtonRef: RefObject<HTMLButtonElement | null>;
  pairDisabled: boolean;
  pairBusy: boolean;
  onPair: () => void;
}

export function DevicesHeaderActions({
  pairButtonRef,
  pairDisabled,
  pairBusy,
  onPair,
}: DevicesHeaderActionsProps) {
  const pairAction = (
    <Button
      ref={pairButtonRef}
      type="button"
      size="sm"
      disabled={pairDisabled}
      aria-busy={pairBusy || undefined}
      aria-label="Connect a device"
      onClick={onPair}
    >
      <Icon name="plus" aria-hidden="true" />
      Connect a device
    </Button>
  );

  return (
    <div className={styles.actions}>
      {pairAction}
    </div>
  );
}
