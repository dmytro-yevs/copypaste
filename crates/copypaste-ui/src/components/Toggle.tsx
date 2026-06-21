// ---------------------------------------------------------------------------
// Toggle — shared iOS-style switch
//
// One canonical toggle used throughout Settings (and elsewhere).
// Focus ring uses the design-system token (focus:ring-ide-accent/50 +
// focus:ring-offset-ide-bg) — NOT a generic Tailwind focus:ring.
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
        // checked = accent fill only, no glow shadow.
        checked
          ? "border-ide-accent bg-ide-accent"
          : "border-ide-border bg-ide-elevated",
      ].join(" ")}
    >
      <span
        className={[
          // Hairline border keeps the white knob visible against a light-theme
          // unchecked track (bg-ide-elevated), where bg-white alone vanished.
          "inline-block h-[12px] w-[12px] rounded-full bg-white border border-ide-border/70 shadow-ide-xs",
          "transition-transform duration-[120ms] ease",
          checked ? "translate-x-[18px]" : "translate-x-[2px]",
        ].join(" ")}
      />
    </button>
  );
}
