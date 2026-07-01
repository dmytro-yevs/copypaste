// ---------------------------------------------------------------------------
// Toggle — shared iOS-style switch
//
// One canonical toggle used throughout Settings (and elsewhere).
// Focus ring uses the design-system token (focus:ring-ide-accent/50 +
// focus:ring-offset-ide-bg) — NOT a generic Tailwind focus:ring.
//
// §MO-9 (CopyPaste-crh3.19): thumb travel and transition are driven by the
// .switch-track / .switch-knob CSS primitives from animations.css.
//   • Track bg: switch-track (off) / switch-track.on (checked) — var(--dur-fast) ease.
//   • Knob:     switch-knob  — translateX(0) → translateX(20px), var(--dur-fast) ease.
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
    >
      <span />
    </button>
  );
}
