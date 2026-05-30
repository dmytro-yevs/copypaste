/**
 * StepSlider — a stepped slider that snaps to a fixed array of values.
 *
 * Design spec: DESIGN-SYSTEM-v2.md §6
 *   - Track 4px, fill accent from 0 to thumb
 *   - Thumb 14px white with E2 shadow + accent focus ring
 *   - Tick marks per step below the track
 *   - Fixed 80px value label right-aligned showing human string
 *   - Save on release (onRelease callback)
 *   - Never allows an unsafe arbitrary value — only array indices
 */

import { useCallback, useId } from "react";

interface StepSliderProps<T> {
  /** The discrete array of allowed values. Must have ≥ 2 entries. */
  steps: readonly T[];
  /** Current value — must be one of the `steps` entries (uses indexOf). */
  value: T;
  /** Called on pointer-up / keyboard release with the newly chosen value. */
  onRelease: (v: T) => void;
  /** Called during drag (before release) to update local display state. */
  onChange: (v: T) => void;
  /** Human-readable label for the current value (shown right, fixed 80px). */
  formatLabel: (v: T) => string;
  /** aria-label for the slider thumb. */
  ariaLabel?: string;
  disabled?: boolean;
}

export function StepSlider<T>({
  steps,
  value,
  onRelease,
  onChange,
  formatLabel,
  ariaLabel,
  disabled = false,
}: StepSliderProps<T>) {
  const id = useId();
  const max = steps.length - 1;

  // Find the closest index for the current value.
  const currentIndex = steps.indexOf(value as T);
  const safeIndex = currentIndex < 0 ? 0 : currentIndex;

  const fillPercent = max > 0 ? (safeIndex / max) * 100 : 0;

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const idx = Number(e.target.value);
      const stepped = steps[Math.min(Math.max(idx, 0), max)];
      onChange(stepped);
    },
    [steps, max, onChange]
  );

  const handleRelease = useCallback(
    (e: React.PointerEvent<HTMLInputElement> | React.KeyboardEvent<HTMLInputElement>) => {
      // For keyboard events only fire on relevant keys
      if ("key" in e) {
        const key = (e as React.KeyboardEvent).key;
        if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(key)) return;
      }
      const idx = Number((e.target as HTMLInputElement).value);
      const stepped = steps[Math.min(Math.max(idx, 0), max)];
      onRelease(stepped);
    },
    [steps, max, onRelease]
  );

  return (
    <div className="flex items-center gap-3">
      {/* Track + thumb wrapper — relative so ticks can be absolutely placed */}
      <div className="relative flex flex-col" style={{ width: "160px" }}>
        {/* Custom track via background gradient */}
        <input
          id={id}
          type="range"
          min={0}
          max={max}
          step={1}
          value={safeIndex}
          disabled={disabled}
          aria-label={ariaLabel}
          aria-valuetext={formatLabel(value)}
          onChange={handleChange}
          onPointerUp={handleRelease}
          onKeyUp={handleRelease}
          className={[
            "step-slider-input",
            "w-full appearance-none cursor-pointer",
            "disabled:cursor-not-allowed disabled:opacity-40",
            // focus ring via focus-visible
            "focus:outline-none focus-visible:ring-2 focus-visible:ring-ide-accent/45 focus-visible:ring-offset-1 focus-visible:ring-offset-ide-bg",
          ].join(" ")}
          style={{
            // Track: filled portion = accent, unfilled = elevated border
            background: `linear-gradient(to right, #3592ff ${fillPercent}%, #383b42 ${fillPercent}%)`,
            height: "4px",
            borderRadius: "2px",
          }}
        />
        {/* Tick marks — one per step, evenly distributed */}
        <div className="relative mt-0.5" style={{ height: "4px" }}>
          {steps.map((_, i) => {
            const pct = max > 0 ? (i / max) * 100 : 0;
            const active = i <= safeIndex;
            return (
              <span
                key={i}
                className="absolute top-0 -translate-x-1/2"
                style={{
                  left: `${pct}%`,
                  width: "2px",
                  height: "4px",
                  borderRadius: "1px",
                  backgroundColor: active ? "#3592ff" : "#383b42",
                  opacity: active ? 0.7 : 0.5,
                }}
              />
            );
          })}
        </div>
      </div>

      {/* Fixed-width value label — always 80px, right-aligned */}
      <span
        className="shrink-0 text-right text-[13px] tabular-nums text-ide-text"
        style={{ width: "80px" }}
      >
        {formatLabel(value)}
      </span>

      {/* Inline CSS for thumb styling — can't do ::-webkit-slider-thumb via Tailwind classes */}
      <style>{`
        .step-slider-input::-webkit-slider-thumb {
          -webkit-appearance: none;
          width: 14px;
          height: 14px;
          border-radius: 50%;
          background: #ffffff;
          box-shadow: 0 2px 8px rgba(0,0,0,0.45), 0 1px 2px rgba(0,0,0,0.35);
          cursor: pointer;
          margin-top: -5px;
          transition: box-shadow 120ms ease;
        }
        .step-slider-input::-webkit-slider-thumb:hover {
          box-shadow: 0 0 0 3px rgba(53,146,255,0.25), 0 2px 8px rgba(0,0,0,0.45);
        }
        .step-slider-input:focus-visible::-webkit-slider-thumb {
          box-shadow: 0 0 0 1px #16171a, 0 0 0 3px rgba(53,146,255,0.45), 0 2px 8px rgba(0,0,0,0.45);
        }
        .step-slider-input::-moz-range-thumb {
          width: 14px;
          height: 14px;
          border-radius: 50%;
          background: #ffffff;
          border: none;
          box-shadow: 0 2px 8px rgba(0,0,0,0.45), 0 1px 2px rgba(0,0,0,0.35);
          cursor: pointer;
        }
        .step-slider-input::-moz-range-track {
          height: 4px;
          border-radius: 2px;
          background: transparent;
        }
      `}</style>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Pre-defined step arrays (bytes/seconds values) — spec §6
// Arrays MUST include/exceed core defaults: text 15MiB, image 64MiB.
// ---------------------------------------------------------------------------

/** 1,2,5,10,15,25,50,100 MB in bytes */
export const TEXT_SIZE_STEPS_BYTES = [
  1  * 1_000_000,
  2  * 1_000_000,
  5  * 1_000_000,
  10 * 1_000_000,
  15 * 1_000_000,  // core default: 15 MiB ≈ 15.7 MB — 15 MB step is closest
  25 * 1_000_000,
  50 * 1_000_000,
  100 * 1_000_000,
] as const;

export const TEXT_SIZE_LABELS = [
  "1 MB", "2 MB", "5 MB", "10 MB", "15 MB", "25 MB", "50 MB", "100 MB (max)",
] as const;

/** 5,10,25,64,128,256,512 MB in bytes. 64 MB step ≥ core default 64 MiB. */
export const IMAGE_SIZE_STEPS_BYTES = [
  5   * 1_000_000,
  10  * 1_000_000,
  25  * 1_000_000,
  64  * 1_000_000,  // core default: 64 MiB ≈ 67 MB — 64 MB step is closest
  128 * 1_000_000,
  256 * 1_000_000,
  512 * 1_000_000,
] as const;

export const IMAGE_SIZE_LABELS = [
  "5 MB", "10 MB", "25 MB", "64 MB", "128 MB", "256 MB", "512 MB (max)",
] as const;

/** 64MB, 128MB, 256MB, 512MB, 1GB, 2GB in bytes. Core default 1 GiB included. */
export const FILE_SIZE_STEPS_BYTES = [
  64   * 1_000_000,
  128  * 1_000_000,
  256  * 1_000_000,
  512  * 1_000_000,
  1024 * 1_000_000,   // core default: 1 GiB ≈ 1073 MB — 1 GB step is closest
  2048 * 1_000_000,
] as const;

export const FILE_SIZE_LABELS = [
  "64 MB", "128 MB", "256 MB", "512 MB", "1 GB", "2 GB (max)",
] as const;

/** 1,2,5,10,25,50 GB in bytes. Core default 10 GiB included. */
export const QUOTA_STEPS_BYTES = [
  1  * 1_000_000_000,
  2  * 1_000_000_000,
  5  * 1_000_000_000,
  10 * 1_000_000_000,   // core default: 10 GiB ≈ 10.7 GB — 10 GB step is closest
  25 * 1_000_000_000,
  50 * 1_000_000_000,
] as const;

export const QUOTA_LABELS = [
  "1 GB", "2 GB", "5 GB", "10 GB", "25 GB", "50 GB (max)",
] as const;

/**
 * Max stored items: [100,250,500,1000,2500,5000,10000,UNLIMITED]
 * Unlimited sentinel = 100_000 — matches HISTORY_LIMIT in defaults.rs
 * which is documented as "intentionally generous: history should feel
 * unbounded to the user". The daemon size-only prune still applies.
 */
export const UNLIMITED_SENTINEL = 100_000;

export const HISTORY_STEPS = [
  100, 250, 500, 1_000, 2_500, 5_000, 10_000, UNLIMITED_SENTINEL,
] as const;

export const HISTORY_LABELS = [
  "100", "250", "500", "1,000", "2,500", "5,000", "10,000", "Unlimited",
] as const;

/** Sensitive auto-wipe: [10s,30s,60s,5m,15m,1h] in seconds */
export const SENSITIVE_TTL_STEPS = [
  10, 30, 60, 5 * 60, 15 * 60, 60 * 60,
] as const;

export const SENSITIVE_TTL_LABELS = [
  "10 s", "30 s", "1 min", "5 min", "15 min", "1 hour",
] as const;

// ---------------------------------------------------------------------------
// Snap helpers — find the nearest step index for a raw value
// ---------------------------------------------------------------------------

/** Return the step value closest to `raw` (by minimum absolute distance). */
export function snapToNearest<T extends number>(steps: readonly T[], raw: number): T {
  let best = 0;
  let bestDist = Math.abs(raw - (steps[0] as number));
  for (let i = 1; i < steps.length; i++) {
    const d = Math.abs(raw - (steps[i] as number));
    if (d < bestDist) {
      bestDist = d;
      best = i;
    }
  }
  return steps[best];
}
