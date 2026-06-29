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
   * crh3.43: canonical section-label colour is text-ide-dim per PARITY-SPEC §3.
   * Android Components.kt:544 uses c.dim (not c.faint — bdac.89 comment was wrong).
   * Default is false (→ text-ide-dim). Pass faint={true} only for deliberately
   * lighter decorative labels (non-standard; deviates from spec).
   */
  faint?: boolean;
}

export function SectionHeader({ label, hint, faint = false }: SectionHeaderProps) {
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
