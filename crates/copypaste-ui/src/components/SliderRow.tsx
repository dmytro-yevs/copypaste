// ---------------------------------------------------------------------------
// SliderRow — consistent grid: [slider (flex)] [fixed-width value]
//
// W4-2: Extended in v0.5.3 with optional onRelease to save only on mouse-up/
// touch-end (prevents spamming IPC on every drag tick in storage sliders).
// ---------------------------------------------------------------------------

interface SliderRowProps {
  min: number;
  max: number;
  step: number;
  value: number;
  onChange: (v: number) => void;
  /** Called on mouse-up / touch-end / key-up — saves to daemon without spamming. */
  onRelease?: (v: number) => void;
  /** Format the numeric value for the right-hand value label. */
  formatValue: (v: number) => string;
  disabled?: boolean;
  /** When provided, renders a <datalist> with this many tick options so browsers show step ticks. */
  tickStepCount?: number;
}

export function SliderRow({
  min,
  max,
  step,
  value,
  onChange,
  onRelease,
  formatValue,
  disabled,
  tickStepCount,
}: SliderRowProps) {
  // Generate a stable id for the datalist when tick marks are requested.
  // We use the min/max/step combo as a cheap content-stable key.
  const datalistId =
    tickStepCount !== undefined ? `slider-ticks-${min}-${max}-${step}` : undefined;

  // Build tick option values for the datalist — one per step index.
  const tickOptions =
    datalistId !== undefined
      ? Array.from({ length: tickStepCount! }, (_, i) =>
          min + i * ((max - min) / Math.max(tickStepCount! - 1, 1)),
        )
      : [];

  return (
    <div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        list={datalistId}
        onChange={(e) => onChange(Number(e.target.value))}
        onMouseUp={(e) => onRelease?.(Number((e.target as HTMLInputElement).value))}
        onTouchEnd={(e) => onRelease?.(Number((e.currentTarget as HTMLInputElement).value))}
        onKeyUp={(e) => onRelease?.(Number((e.target as HTMLInputElement).value))}
      />
      {/* §6.5: datalist provides step tick marks rendered by the browser */}
      {datalistId !== undefined && (
        <datalist id={datalistId}>
          {tickOptions.map((v) => (
            <option key={v} value={v} />
          ))}
        </datalist>
      )}
      {/* §6.4: w-[80px] (was w-[52px]) so longer labels like "Unlimited" fit */}
      <span>{formatValue(value)}</span>
    </div>
  );
}
