// ---------------------------------------------------------------------------
// Panel — settings row-group wrapper
//
// Always paired with a sibling SectionHeader (which renders .set-grp__h) —
// Panel itself supplies the `.set-grp` grouping box (shell.css: 22px
// margin-bottom) around a stack of SettingsRow `.srow` children. Matches
// the reference markup's flat `<div class="set-grp"><div class="set-grp-h">…
// <div class="srow">…` shape: no card/border surface here — `.srow` borders
// (border-bottom divider, last-child:0) already delineate rows, and
// `.set-body`'s own padding provides the horizontal inset.
// ---------------------------------------------------------------------------

interface PanelProps {
  children: React.ReactNode;
}

export function Panel({ children }: PanelProps) {
  return (
    // CopyPaste-8ebg.35: bare div gave no ARIA grouping, so the row-group had
    // no boundary for the rotor. role="group" matches the pattern already
    // used elsewhere in this codebase (GalleryView, DisplayTab — role="group"
    // aria-label="…"). Panel is always paired with a sibling SectionHeader
    // that now renders role="heading" (see SectionHeader.tsx); Panel and
    // SectionHeader are siblings, not nested, so there is no shared id to
    // wire into aria-labelledby without also touching every call site — out
    // of scope for this fix (touch-only-these-2-files constraint). An
    // unnamed group still gives AT a real boundary instead of an opaque div.
    <div className="set-grp" role="group">
      {children}
    </div>
  );
}
