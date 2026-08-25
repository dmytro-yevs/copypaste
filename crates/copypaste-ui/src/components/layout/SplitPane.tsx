import { useEffect, useRef, type KeyboardEvent, type ReactNode } from "react";
import {
  Group,
  Panel,
  Separator,
  type PanelImperativeHandle,
} from "react-resizable-panels";

import styles from "./SplitPane.module.css";

interface SplitPaneProps {
  primary: ReactNode;
  secondary?: ReactNode;
  primaryId?: string;
  secondaryId?: string;
  primaryMinSize?: string | number;
  secondaryDefaultSize?: string | number;
  secondarySize?: string | number;
  secondaryMinSize?: string | number;
  secondaryMaxSize?: string | number;
  secondaryCollapsible?: boolean;
  secondaryCollapsedSize?: string | number;
  separatorLabel?: string;
  onSecondarySizeChange?: (pixels: number) => void;
}

export function SplitPane({
  primary,
  secondary,
  primaryId = "primary-pane",
  secondaryId = "secondary-pane",
  primaryMinSize = 0,
  secondaryDefaultSize = "50%",
  secondarySize,
  secondaryMinSize = 0,
  secondaryMaxSize = "100%",
  secondaryCollapsible = false,
  secondaryCollapsedSize = 0,
  separatorLabel = "Resize panels",
  onSecondarySizeChange,
}: SplitPaneProps) {
  const secondaryRef = useRef<PanelImperativeHandle | null>(null);
  useEffect(() => {
    if (secondarySize !== undefined) secondaryRef.current?.resize(secondarySize);
  }, [secondarySize]);
  const resizeFromKeyboard = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    const panel = secondaryRef.current;
    if (!panel) return;
    event.preventDefault();
    event.stopPropagation();
    const direction = event.key === "ArrowLeft" ? 1 : -1;
    const step = event.shiftKey ? 32 : 8;
    panel.resize(`${panel.getSize().inPixels + direction * step}px`);
  };

  return (
    <Group
      orientation="horizontal"
      resizeTargetMinimumSize={{ fine: 12, coarse: 24 }}
      className={styles.group}
    >
      <Panel id={primaryId} className={styles.panel} minSize={primaryMinSize}>
        {primary}
      </Panel>
      {secondary === undefined ? null : (
        <>
          <Separator
            data-slot="split-pane-separator"
            className={styles.separator}
            aria-label={separatorLabel}
            onKeyDown={resizeFromKeyboard}
          />
          <Panel
            id={secondaryId}
            className={styles.panel}
            defaultSize={secondaryDefaultSize}
            minSize={secondaryMinSize}
            maxSize={secondaryMaxSize}
            collapsible={secondaryCollapsible}
            collapsedSize={secondaryCollapsedSize}
            panelRef={secondaryRef}
            onResize={(size) => onSecondarySizeChange?.(size.inPixels)}
          >
            {secondary}
          </Panel>
        </>
      )}
    </Group>
  );
}
