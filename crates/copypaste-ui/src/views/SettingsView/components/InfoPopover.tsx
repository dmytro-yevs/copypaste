// InfoPopover.tsx
// Extracted from SettingsView.tsx (CopyPaste-g06m.14 split) — cut/paste only.
//
// HW-M3 fix: popover content is rendered via ReactDOM.createPortal to
// document.body so it can never be clipped by an ancestor overflow-hidden div.
// Position is computed from the trigger button's getBoundingClientRect.
// Click outside to close.
import { useCallback, useEffect, useRef, useState } from "react";
import ReactDOM from "react-dom";

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
        <div
          ref={popoverRef}
          className="surface-glass-strong z-[9999] w-56 p-2 text-[11px] text-ide-dim"
          style={{
            position: "fixed",
            top: pos.top,
            left: pos.left,
            minWidth: "14rem",
            transform: "translateY(-50%)",
            borderRadius: "var(--r-ctl)",
          }}
        >
          {text}
        </div>,
        document.body
      )
    : null;

  return (
    <div className="inline-flex items-center">
      <button
        ref={btnRef}
        type="button"
        aria-label="More info"
        aria-expanded={open}
        onClick={handleToggle}
        className="flex h-6 w-6 items-center justify-center rounded-full text-ide-faint hover:text-ide-dim transition-colors"
      >
        <svg viewBox="0 0 16 16" width="13" height="13" fill="currentColor" aria-hidden="true">
          <path d="M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1Zm0 3a.9.9 0 1 1 0 1.8A.9.9 0 0 1 8 4Zm-.75 2.75h1.5v4.5h-1.5v-4.5Z" />
        </svg>
      </button>
      {popoverEl}
    </div>
  );
}
