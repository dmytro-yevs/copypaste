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
          11px semibold uppercase matching Components.kt SectionLabel.
          crh3.43: faint vs dim is now moot — .set-grp__h always renders in
          var(--faint) per shell.css, so the faint prop has no visual axis left.
          CopyPaste-8ebg.35: bare div gave no heading semantics, so the VoiceOver
          rotor's Headings category skipped every section. role="heading" +
          aria-level (no visual/CSS change) makes it a real heading landmark;
          level 3 since it sits below the tab/view title (level-2 equivalent). */}
      <div className="set-grp__h" role="heading" aria-level={3}>
        {label}
      </div>
      {/* No dedicated contract class for a group hint — reuse .srow__s (small,
          faint, max-width text) since it matches this role exactly. */}
      {hint && <div className="srow__s">{hint}</div>}
    </div>
  );
}
