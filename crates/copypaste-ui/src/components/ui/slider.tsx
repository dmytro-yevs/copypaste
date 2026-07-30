import { type ComponentProps, useMemo } from "react";
import * as SliderPrimitive from "@radix-ui/react-slider";

import { cn } from "@/lib/cn";

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
      className={cn(
        "relative flex w-full touch-none items-center select-none data-[disabled]:opacity-50",
        className,
      )}
      {...props}
    >
      <SliderPrimitive.Track
        data-slot="slider-track"
        className="relative h-1.5 w-full grow overflow-hidden rounded-full bg-muted"
      >
        <SliderPrimitive.Range
          data-slot="slider-range"
          className="absolute h-full bg-primary"
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
          className="block size-4 shrink-0 rounded-full border border-primary bg-background shadow-sm transition-[color,box-shadow] hover:ring-4 hover:ring-ring focus-visible:ring-4 focus-visible:ring-ring focus-visible:outline-hidden disabled:pointer-events-none"
        />
      ))}
    </SliderPrimitive.Root>
  );
}

export { Slider };
