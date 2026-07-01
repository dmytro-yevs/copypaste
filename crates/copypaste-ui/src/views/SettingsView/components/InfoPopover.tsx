// InfoPopover.tsx
// Extracted from SettingsView.tsx (CopyPaste-g06m.14 split) — cut/paste only.
//
// HW-M3 fix: popover content is rendered via ReactDOM.createPortal to
// document.body so it can never be clipped by an ancestor overflow-hidden div.
// Position is computed from the trigger button's getBoundingClientRect.
// Click outside to close.
import { useCallback, useEffect, useRef, useState } from "react";
import ReactDOM from "react-dom";
import { Info } from "lucide-react";

export function InfoPopover({ text }: { text: string }) {
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<{ top: number; left: number }>({ top: 0, left: 0 });
  const btnRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  // Recompute position from the trigger button each time it opens.
  const handleToggle = useCallback(() => {
    if (!open && btnRef.current) {
      const rect = btnRef.current.getBoundingClientRect();
      // Place popover to the right of the icon, vertically centered on it.
      setPos({
        top: rect.top + rect.height / 2,
        left: rect.right + 6,
      });
    }
    setOpen((v) => !v);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as Node;
      const outsideBtn = btnRef.current && !btnRef.current.contains(target);
      const outsidePopover = popoverRef.current && !popoverRef.current.contains(target);
      if (outsideBtn && outsidePopover) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const popoverEl = open
    ? ReactDOM.createPortal(
        // Position is functionally required: computed from the trigger button's
        // getBoundingClientRect so the popover anchors correctly (kept per de-style pass).
        <div
          ref={popoverRef}
          className="srow__s"
          style={{
            position: "fixed",
            top: pos.top,
            left: pos.left,
            transform: "translateY(-50%)",
          }}
        >
          {text}
        </div>,
        document.body
      )
    : null;

  return (
    <div>
      <button
        ref={btnRef}
        type="button"
        className="iconbtn"
        aria-label="More info"
        aria-expanded={open}
        onClick={handleToggle}
      >
        <Info aria-hidden="true" />
      </button>
      {popoverEl}
    </div>
  );
}
