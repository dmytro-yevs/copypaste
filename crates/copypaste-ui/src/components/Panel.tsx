// ---------------------------------------------------------------------------
// Panel — frosted-glass card wrapper for settings sections
//
// HW-M3 note: overflow-hidden on the inner div clips bottom-borders to the
// panel's rounded corners without clipping absolutely-positioned InfoPopover
// (z-50), which floats above the outer div via z-50.
//
// surface-card = frosted translucent glass: reads --skin-* vars for material,
// blur, fill, shadow (--skin-shadow-card) so panels adapt to the active skin.
//   Classic:  e2 shadow + glass
//   Quiet:    no shadow + flat
//   Vapor:    no card shadow + sheen
// ---------------------------------------------------------------------------

interface PanelProps {
  children: React.ReactNode;
}

export function Panel({ children }: PanelProps) {
  return (
    <div className="surface-card" style={{ borderRadius: "var(--skin-r-card)" }}>
      <div className="overflow-hidden" style={{ borderRadius: "var(--skin-r-card)" }}>
        {children}
      </div>
    </div>
  );
}
