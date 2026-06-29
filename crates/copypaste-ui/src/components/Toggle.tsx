// ---------------------------------------------------------------------------
// Toggle — shared iOS-style switch
//
// One canonical toggle used throughout Settings (and elsewhere).
// Focus ring uses the design-system token (focus:ring-ide-accent/50 +
// focus:ring-offset-ide-bg) — NOT a generic Tailwind focus:ring.
//
// §MO-9 (CopyPaste-crh3.19): thumb travel and transition are driven by the
// .switch-track / .switch-knob CSS primitives from animations.css.
//   • Track bg: switch-track (off) / switch-track.on (checked) — var(--mo-fast) ease.
//   • Knob:     switch-knob  — translateX(0) → translateX(20px), var(--mo-fast) ease.
// Hardcoded translate-x-[16px] / duration-[120ms] removed; tokens govern both.
// ---------------------------------------------------------------------------

interface ToggleProps {
  checked: boolean;
  onChange: (val: boolean) => void;
  disabled?: boolean;
  "aria-label"?: string;
}

export function Toggle({
  checked,
  onChange,
  disabled,
  "aria-label": ariaLabel,
}: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={[
        "relative inline-flex h-[18px] w-[34px] shrink-0 cursor-pointer items-center rounded-full",
        "border focus:outline-none focus:ring-2 focus:ring-ide-accent/50 focus:ring-offset-1 focus:ring-offset-ide-bg",
        "disabled:cursor-not-allowed disabled:opacity-40",
        // §MO-9: track bg + transition via CSS primitive (.switch-track / .switch-track.on).
        // Border colour is kept conditional so the hairline reads correctly in both states.
        "switch-track",
        checked ? "on border-ide-accent" : "border-ide-border",
      ].join(" ")}
    >
      <span
        className={[
          // Hairline border keeps the white knob visible against a light-theme
          // unchecked track, where bg-white alone vanished.
          "inline-block h-[12px] w-[12px] rounded-full bg-white border border-ide-border/70 shadow-ide-xs",
          // §MO-9: thumb travel translateX(20px) + transition via CSS primitive.
          // Replaces the old inline duration-[120ms] ease translate-x-[16px] magic numbers.
          "switch-knob",
        ].join(" ")}
      />
    </button>
  );
}
