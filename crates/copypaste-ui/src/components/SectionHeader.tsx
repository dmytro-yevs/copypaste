import { useUI } from "../store";

// ---------------------------------------------------------------------------
// SectionHeader — shared section / subsection label
//
// Mirrors SubsectionHeader from SettingsView.tsx (CopyPaste-zxv2 Step 3).
// 11px semibold uppercase text-ide-dim, density-aware top margin, first:mt-0.
// Also used in DevicesView for "Paired Devices" / "Discovered on your network"
// labels (replaces raw <p> tags there).
// ---------------------------------------------------------------------------

interface SectionHeaderProps {
  label: string;
  hint?: string;
  /**
   * bdac.89: canonical section-label colour is text-ide-faint, matching
   * Android SectionLabel (Components.kt:1256 — c.faint per PARITY-SPEC §3).
   * Default changed to true so all section headers are faint by default.
   * Pass faint={false} explicitly only when a higher-contrast dim label
   * is intentionally needed (non-standard use case).
   */
  faint?: boolean;
}

export function SectionHeader({ label, hint, faint = true }: SectionHeaderProps) {
  // CopyPaste-hffp: tighter top margin in compact density to reduce whitespace.
  const density = useUI((s) => s.prefs.density ?? "comfortable");
  const mt =
    density === "compact"
      ? "mt-5"
      : density === "spacious"
        ? "mt-9"
        : "mt-7";
  return (
    <div className={`${mt} mb-1.5 first:mt-0`}>
      {/* §3: section labels = grey (text-ide-dim or text-ide-faint), NOT accent blue;
          11px semibold uppercase matching Components.kt SectionLabel. */}
      <div
        className={[
          "text-[11px] font-semibold uppercase tracking-wider",
          faint ? "text-ide-faint" : "text-ide-dim",
        ].join(" ")}
      >
        {label}
      </div>
      {hint && <div className="mt-0.5 text-[11px] text-ide-faint">{hint}</div>}
    </div>
  );
}
