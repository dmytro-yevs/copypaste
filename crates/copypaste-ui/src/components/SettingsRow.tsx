import { useUI } from "../store";

// ---------------------------------------------------------------------------
// SettingsRow — one canonical settings row
//
// Props:
//   title        — row label (sentence case). Rendered left-aligned.
//   description  — optional secondary text shown below the title.
//   children     — right-aligned control slot (Toggle, slider, input, button…)
//   disabled     — visually dims the title/description when true.
//
// Density-aware: compact / comfortable / spacious row heights match the rest
// of the Settings UI (CopyPaste-hffp / CopyPaste-gzli).
// ---------------------------------------------------------------------------

interface SettingsRowProps {
  title: string;
  description?: string;
  /** Right-aligned control area. */
  children: React.ReactNode;
  disabled?: boolean;
}

export function SettingsRow({ title, description, children, disabled }: SettingsRowProps) {
  // CopyPaste-hffp: density-aware row height/padding.
  // CopyPaste-gzli: spacious adds extra padding as the largest step.
  const density = useUI((s) => s.prefs.density ?? "comfortable");
  const rowCls =
    density === "compact"
      ? "flex min-h-[30px] items-center justify-between border-b border-ide-divider/70 px-3 py-1 last:border-b-0"
      : density === "spacious"
        ? "flex min-h-[42px] items-center justify-between border-b border-ide-divider/70 px-3 py-2.5 last:border-b-0"
        : "flex min-h-[36px] items-center justify-between border-b border-ide-divider/70 px-3 py-2 last:border-b-0";

  return (
    <div className={rowCls}>
      {/* Left: title + optional description */}
      <div
        className={[
          "min-w-[160px] shrink-0",
          disabled ? "opacity-40" : "",
        ].join(" ")}
      >
        {/* W4-3: fixed min-width on label column prevents wrapping on narrow labels */}
        <span className="text-[13px] text-ide-text">{title}</span>
        {description && (
          <p className="mt-0.5 text-[11px] text-ide-faint">{description}</p>
        )}
      </div>
      {/* Right: control slot */}
      <div className="flex items-center gap-2">{children}</div>
    </div>
  );
}
