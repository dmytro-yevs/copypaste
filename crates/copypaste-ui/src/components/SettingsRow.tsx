// ---------------------------------------------------------------------------
// SettingsRow — one canonical settings row
//
// Props:
//   title        — row label (sentence case). Rendered left-aligned.
//   description  — optional secondary text shown below the title.
//   info         — optional node (e.g. InfoPopover) rendered inline after the
//                  title in the left/label column. bdac.104: info belongs next
//                  to the label, not inside the control column.
//   children     — right-aligned control slot (Toggle, slider, input, button…)
//   disabled     — visually dims the title/description when true.
// ---------------------------------------------------------------------------

interface SettingsRowProps {
  title: string;
  description?: string;
  /** Optional node (e.g. InfoPopover) shown inline after the title label. */
  info?: React.ReactNode;
  /** Right-aligned control area. */
  children: React.ReactNode;
  disabled?: boolean;
  /**
   * When true, switches to a stacked layout: title on top, children below
   * spanning full width. Use for wide controls (grids, multi-column pickers)
   * that do not fit in the two-column label/control layout. bdac.105.
   */
  fullWidth?: boolean;
}

export function SettingsRow({ title, description, info, children, disabled, fullWidth }: SettingsRowProps) {
  return (
    <div
      className="srow"
      // fullWidth: stacked layout (title on top, control below spanning full
      // width) for wide controls that don't fit the two-column label/control
      // layout — no dedicated contract class exists for this variant, so the
      // stack is expressed inline (design.md's .srow is otherwise row-only).
      style={
        fullWidth
          ? { flexDirection: "column", alignItems: "flex-start", justifyContent: "flex-start", gap: "var(--s-3)" }
          : undefined
      }
    >
      {/* Left/top: title + optional description + optional info icon */}
      <div className="srow__l" style={disabled ? { opacity: 0.5 } : undefined}>
        {/* W4-3: fixed min-width on label column prevents wrapping on narrow labels */}
        {/* bdac.104: info slot rendered inline after title — stays in label column */}
        <div>
          <span>{title}</span>
          {info}
        </div>
        {description && <p className="srow__s">{description}</p>}
      </div>
      {/* Right/below: control slot */}
      <div className="srow__c" style={fullWidth ? { width: "100%" } : undefined}>{children}</div>
    </div>
  );
}
