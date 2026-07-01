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
    <div className="set-grp">
      {children}
    </div>
  );
}
