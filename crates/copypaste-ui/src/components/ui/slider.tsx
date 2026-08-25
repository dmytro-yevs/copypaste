import { type ComponentProps, useMemo } from "react";
import * as SliderPrimitive from "@radix-ui/react-slider";

import { cn } from "@/lib/cn";
import styles from "./slider.module.css";

function Slider({
  className,
  defaultValue,
  value,
  min = 0,
  max = 100,
  "aria-label": ariaLabel,
  ...props
}: ComponentProps<typeof SliderPrimitive.Root>) {
  const values = useMemo(
    () =>
      Array.isArray(value)
        ? value
        : Array.isArray(defaultValue)
          ? defaultValue
          : [min, max],
    [value, defaultValue, min, max],
  );

  return (
    <SliderPrimitive.Root
      data-slot="slider"
      defaultValue={defaultValue}
      value={value}
      min={min}
      max={max}
      className={cn(styles.root, className)}
      {...props}
    >
      <SliderPrimitive.Track
        data-slot="slider-track"
        className={styles.track}
      >
        <SliderPrimitive.Range
          data-slot="slider-range"
          className={styles.range}
        />
      </SliderPrimitive.Track>
      {/* The element carrying `role="slider"` is the *thumb*, so the
          accessible name has to be forwarded onto it — a name on the root
          leaves the control itself unnamed to a screen reader (A11Y-9). Stock
          shadcn does not do this; it is the one change we make to the file. */}
      {values.map((_, index) => (
        <SliderPrimitive.Thumb
          data-slot="slider-thumb"
          key={index}
          aria-label={
            ariaLabel === undefined
              ? undefined
              : values.length > 1
                ? `${ariaLabel} ${index + 1}`
                : ariaLabel
          }
          className={styles.thumb}
        />
      ))}
    </SliderPrimitive.Root>
  );
}

export { Slider };
