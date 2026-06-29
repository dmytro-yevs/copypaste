// ---------------------------------------------------------------------------
// Panel — frosted-glass card wrapper for settings sections
//
// HW-M3 note: overflow-hidden on the inner div clips bottom-borders to the
// panel's rounded corners without clipping absolutely-positioned InfoPopover
// (z-50), which floats above the outer div via z-50.
//
// surface-card = frosted translucent glass card with radius and shadow tokens.
// ---------------------------------------------------------------------------

interface PanelProps {
  children: React.ReactNode;
}

export function Panel({ children }: PanelProps) {
  return (
    <div className="surface-card" style={{ borderRadius: "var(--r-card)" }}>
      <div className="overflow-hidden" style={{ borderRadius: "var(--r-card)" }}>
        {children}
      </div>
    </div>
  );
}
