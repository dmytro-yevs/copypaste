// ---------------------------------------------------------------------------
// SectionHeader — shared section / subsection label
//
// Mirrors SubsectionHeader from SettingsView.tsx (CopyPaste-zxv2 Step 3).
// 11px semibold uppercase text-ide-dim, first:mt-0.
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

export function SectionHeader({ label, hint }: SectionHeaderProps) {
  return (
    <div>
      {/* §3: section labels = grey (text-ide-dim or text-ide-faint), NOT accent blue;
          11px semibold uppercase matching Components.kt SectionLabel. */}
      <div>
        {label}
      </div>
      {hint && <div>{hint}</div>}
    </div>
  );
}
