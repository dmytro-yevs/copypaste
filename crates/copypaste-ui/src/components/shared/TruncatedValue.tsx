import * as PopoverPrimitive from "@radix-ui/react-popover";
import { useCallback, useLayoutEffect, useRef, useState } from "react";

import { Button, Tooltip } from "@/components/ui";
import { useObservedElementSize, usePointerKind } from "@/hooks/useViewportMetrics";
import styles from "./TruncatedValue.module.css";

export function TruncatedValue({
  value,
  touchPopover = true,
  copyable = false,
  onCopy,
}: {
  value: string;
  touchPopover?: boolean;
  copyable?: boolean;
  onCopy?: (value: string) => void;
}) {
  const ref = useRef<HTMLSpanElement>(null);
  const [overflowed, setOverflowed] = useState(false);
  const pointer = usePointerKind();
  const { ref: observedRef, width } = useObservedElementSize<HTMLSpanElement>();
  const coarse = pointer === "coarse";
  const shown = value;
  const measure = useCallback(() => {
    const element = ref.current;
    setOverflowed(
      element !== null && element.scrollWidth > element.clientWidth,
    );
  }, []);

  const setRef = useCallback((element: HTMLSpanElement | null) => {
    ref.current = element;
    observedRef(element);
  }, [observedRef]);

  useLayoutEffect(measure, [measure, shown, width]);

  const text = (
    <span ref={setRef} className={styles.value} aria-label={shown}>
      {shown}
    </span>
  );
  if (!overflowed) return text;

  if (!coarse || !touchPopover) {
    return <Tooltip content={value}>{text}</Tooltip>;
  }

  return (
    <PopoverPrimitive.Root>
      <PopoverPrimitive.Trigger asChild>
        <Button variant="ghost" className={styles.trigger} aria-label={value}>
          {text}
        </Button>
      </PopoverPrimitive.Trigger>
      <PopoverPrimitive.Portal>
        <PopoverPrimitive.Content
          sideOffset={8}
          collisionPadding={8}
          className={styles.popover}
        >
          <p className={styles.full}>{value}</p>
          {copyable && onCopy ? (
            <Button variant="secondary" icon="copy" onClick={() => onCopy(value)}>
              Copy
            </Button>
          ) : null}
          <PopoverPrimitive.Arrow className={styles.arrow} />
        </PopoverPrimitive.Content>
      </PopoverPrimitive.Portal>
    </PopoverPrimitive.Root>
  );
}
