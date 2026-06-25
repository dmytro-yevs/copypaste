import { useUI } from "../store";

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
//
// Density-aware: compact / comfortable / spacious row heights match the rest
// of the Settings UI (CopyPaste-hffp / CopyPaste-gzli).
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
  // CopyPaste-hffp: density-aware row height/padding.
  // CopyPaste-gzli: spacious adds extra padding as the largest step.
  const density = useUI((s) => s.prefs.density ?? "comfortable");
  // fullWidth rows use block layout (py-3 matches the old raw-div palette wrapper).
  const rowCls = fullWidth
    ? "block border-b border-ide-divider/70 px-3 py-3 last:border-b-0"
    : density === "compact"
      ? "flex min-h-[30px] items-center justify-between border-b border-ide-divider/70 px-3 py-1 last:border-b-0"
      : density === "spacious"
        ? "flex min-h-[42px] items-center justify-between border-b border-ide-divider/70 px-3 py-2.5 last:border-b-0"
        : "flex min-h-[36px] items-center justify-between border-b border-ide-divider/70 px-3 py-2 last:border-b-0";

  return (
    <div className={rowCls}>
      {/* Left/top: title + optional description + optional info icon */}
      <div
        className={[
          fullWidth ? "mb-2" : "min-w-[160px] shrink-0",
          disabled ? "opacity-40" : "",
        ].join(" ")}
      >
        {/* W4-3: fixed min-width on label column prevents wrapping on narrow labels */}
        {/* bdac.104: info slot rendered inline after title — stays in label column */}
        <div className="flex items-center gap-1">
          <span className="text-[13px] text-ide-text">{title}</span>
          {info}
        </div>
        {description && (
          <p className="mt-0.5 text-[11px] text-ide-faint">{description}</p>
        )}
      </div>
      {/* Right/below: control slot */}
      <div className={fullWidth ? "" : "flex items-center gap-2"}>{children}</div>
    </div>
  );
}
