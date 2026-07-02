// InfoPopover.tsx
// Extracted from SettingsView.tsx (CopyPaste-g06m.14 split) — cut/paste only.
//
// HW-M3 fix: popover content is rendered via ReactDOM.createPortal to
// document.body so it can never be clipped by an ancestor overflow-hidden div.
// Position is computed from the trigger button's getBoundingClientRect.
// Click outside to close.
//
// CopyPaste-g27b.35: added flip/collision detection. The original
// implementation always centred the popover vertically on the trigger with a
// fixed offset — its bottom edge could extend into the control the popover
// documents (e.g. the "Excluded apps" textarea sitting directly below the
// label row in a `fullWidth` SettingsRow: popover rect 604..680 overlapped the
// input's 672..692). It now does a two-pass measure: mount invisibly first,
// measure the popover's real size plus the trigger's and the row's control
// (`.srow__c` — every InfoPopover in the app lives inside a SettingsRow's
// `info=` slot, see SettingsRow.tsx) rects, then place it below only when that
// fits before the control (or the viewport bottom when there's no row),
// falling back to above the trigger otherwise.
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import ReactDOM from "react-dom";
import { Info } from "lucide-react";

const GAP = 6;
const VIEWPORT_MARGIN = 8;

interface Pos {
  top: number;
  left: number;
}

export function InfoPopover({ text }: { text: string }) {
  const [open, setOpen] = useState(false);
  // True once the popover's real size has been measured and a collision-aware
  // position has been computed. False right after opening — the popover is
  // rendered invisibly during that single measure pass so there is no visible
  // flash at the wrong (stale/zeroed) position.
  const [positioned, setPositioned] = useState(false);
  const [pos, setPos] = useState<Pos>({ top: 0, left: 0 });
  const btnRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  const handleToggle = useCallback(() => {
    if (!open) setPositioned(false);
    setOpen((v) => !v);
  }, [open]);

  // Two-pass positioning: once the (invisible) popover panel is in the DOM,
  // measure it for real and flip/clamp so it never overlaps the viewport
  // bottom OR the control this popover documents.
  useLayoutEffect(() => {
    if (!open || positioned) return;
    const btn = btnRef.current;
    const panel = popoverRef.current;
    if (!btn || !panel) return;

    const btnRect = btn.getBoundingClientRect();
    const panelRect = panel.getBoundingClientRect();
    const viewportH = window.innerHeight;
    const viewportW = window.innerWidth;

    // The control this popover explains is the sibling `.srow__c` slot inside
    // the same SettingsRow (see SettingsRow.tsx: title+info in `.srow__l`,
    // control in `.srow__c`). When present, its top edge is a hard "never
    // cover this" boundary in addition to the viewport edge.
    const row = btn.closest(".srow");
    const control = row?.querySelector<HTMLElement>(".srow__c") ?? null;
    const belowLimit = control
      ? control.getBoundingClientRect().top
      : viewportH - VIEWPORT_MARGIN;

    const spaceBelow = belowLimit - btnRect.bottom - GAP;
    const spaceAbove = btnRect.top - GAP;
    const fitsBelow = panelRect.height <= spaceBelow;
    const fitsAbove = panelRect.height <= spaceAbove;

    // Prefer below (the original placement) when it fits; flip above when it
    // doesn't but there IS room above; otherwise fall back to below, clamped
    // to the viewport bottom (best effort for the rare case neither side has
    // enough room, e.g. a very short window).
    let top: number;
    if (fitsBelow || !fitsAbove) {
      top = btnRect.bottom + GAP;
      const maxTop = viewportH - VIEWPORT_MARGIN - panelRect.height;
      if (top > maxTop) top = Math.max(VIEWPORT_MARGIN, maxTop);
    } else {
      top = btnRect.top - GAP - panelRect.height;
    }

    let left = btnRect.right + GAP;
    if (left + panelRect.width + VIEWPORT_MARGIN > viewportW) {
      left = Math.max(VIEWPORT_MARGIN, viewportW - panelRect.width - VIEWPORT_MARGIN);
    }

    setPos({ top, left });
    setPositioned(true);
  }, [open, positioned]);

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
        // and the row's control's getBoundingClientRect so the popover anchors
        // correctly and never covers the control it documents (g27b.35).
        <div
          ref={popoverRef}
          className="popover"
          role="tooltip"
          style={{
            position: "fixed",
            top: pos.top,
            left: pos.left,
            // Hidden during the single measure pass so there's no flash at the
            // stale/zeroed position — becomes visible the instant it's placed.
            visibility: positioned ? "visible" : "hidden",
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
