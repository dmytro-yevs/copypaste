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

export function SettingsRow({ title, description, info, children }: SettingsRowProps) {
  return (
    <div>
      {/* Left/top: title + optional description + optional info icon */}
      <div>
        {/* W4-3: fixed min-width on label column prevents wrapping on narrow labels */}
        {/* bdac.104: info slot rendered inline after title — stays in label column */}
        <div>
          <span>{title}</span>
          {info}
        </div>
        {description && <p>{description}</p>}
      </div>
      {/* Right/below: control slot */}
      <div>{children}</div>
    </div>
  );
}
