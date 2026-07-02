import { useContext, type CSSProperties } from "react";
import { SettingsFieldLabelContext } from "./settingsFieldLabelContext";

// ---------------------------------------------------------------------------
// SliderRow — consistent grid: [slider (flex)] [fixed-width value]
//
// W4-2: Extended in v0.5.3 with optional onRelease to save only on mouse-up/
// touch-end (prevents spamming IPC on every drag tick in storage sliders).
//
// CopyPaste-g27b.21: every slider in the app (History display limit,
// Sensitive auto-wipe, Preview lines, Image preview height, Max clip file
// size, Local storage limit, ...) renders through this one component so they
// look and behave identically. Two things are standardized here:
//
//  1. Filled track: native `accent-color` alone only reliably tints the
//     thumb (and, inconsistently across engines, the "filled" portion of the
//     track), so we additionally compute the value's position as a percent
//     and set it as the `--range-fill` custom property inline on the input.
//     primitives.css turns that into a `linear-gradient` on the track so the
//     filled/unfilled split is pixel-accurate and identical in every engine
//     (and legible to jsdom-based tests, which can read the inline style but
//     can't observe native track/thumb painting). `accent-color` is kept as
//     a same-hue fallback for any engine that ignores the gradient rules.
//  2. Tick marks: `tickStepCount` is still accepted (StorageTab's
//     LimitSliderRow passes it) so its type stays stable, but the datalist is
//     no longer wired to the input via the `list` attribute. Native tick
//     rendering for `list`-linked range inputs is inconsistent across
//     engines, which made only *some* sliders (the ones that happened to
//     pass tickStepCount) show ticks — the opposite of "identical". Every
//     slider now renders tick-free.
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
  /**
   * Historically: when provided, renders a <datalist> with this many tick
   * options so browsers show step ticks. Kept for API compatibility with
   * existing callers; see CopyPaste-g27b.21 note above — ticks are no longer
   * visually wired so every slider looks the same.
   */
  tickStepCount?: number;
  /**
   * Explicit accessible name for the range input. When omitted, the enclosing
   * SettingsRow title is used (g27b.26) — a bare range input otherwise has no
   * accessible name and axe flags `label` (critical).
   */
  ariaLabel?: string;
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
  ariaLabel,
}: SliderRowProps) {
  const rowLabel = useContext(SettingsFieldLabelContext);
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

  // Percent of the track that should render as "filled" (accent-colored),
  // clamped defensively in case a caller passes an out-of-range value.
  const fillPct =
    max > min ? Math.min(100, Math.max(0, ((value - min) / (max - min)) * 100)) : 0;

  return (
    // Slider layout/theming lives in primitives.css: `.range` paints the
    // filled/unfilled track from the `--range-fill` percent set below, themes
    // the thumb, and keeps `accent-color` as a fallback; `.range__value`
    // sizes the readout. The row is a token-driven `.ctl` cluster.
    <div className="ctl ctl--field">
      <input
        type="range"
        className="range"
        aria-label={ariaLabel ?? rowLabel}
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        style={{ "--range-fill": `${fillPct}%` } as CSSProperties}
        onChange={(e) => onChange(Number(e.target.value))}
        onMouseUp={(e) => onRelease?.(Number((e.target as HTMLInputElement).value))}
        onTouchEnd={(e) => onRelease?.(Number((e.currentTarget as HTMLInputElement).value))}
        onKeyUp={(e) => onRelease?.(Number((e.target as HTMLInputElement).value))}
      />
      {/* Not wired via `list=` (see file header) — kept so the datalist
          generation logic/id stays stable if tick marks are ever reinstated. */}
      {datalistId !== undefined && (
        <datalist id={datalistId}>
          {tickOptions.map((v) => (
            <option key={v} value={v} />
          ))}
        </datalist>
      )}
      {/* §6.4: min-width 80px (was 52px) so longer labels like "Unlimited" fit */}
      <span className="range__value">
        {formatValue(value)}
      </span>
    </div>
  );
}
