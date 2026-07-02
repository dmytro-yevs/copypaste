import {
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { useFocusTrap } from "../useFocusTrap";
import { acquireScrollLock, releaseScrollLock } from "./scrollLock";

export interface DialogProps {
  /** id of the title element, wired to `aria-labelledby`. */
  labelledBy?: string;
  /** id of the body/description element, wired to `aria-describedby`. */
  describedBy?: string;
  /** Called on Escape and on backdrop click (per the dismissal flags below). */
  onClose: () => void;
  /** Dismiss when the scrim (backdrop) is clicked. Default true. */
  dismissOnBackdrop?: boolean;
  /** Dismiss when Escape is pressed. Default true. */
  dismissOnEscape?: boolean;
  /** Extra class(es) appended to the `.modal` panel. */
  className?: string;
  children: ReactNode;
}

/**
 * Shared modal Dialog primitive (design.md Decision 5). Composes the existing
 * `useFocusTrap` (initial focus on the first focusable descendant or the
 * container fallback, Tab/Shift+Tab cycling, Escape delegation, and focus
 * restoration to the trigger on close — all UNCHANGED) and adds: portal to
 * `document.body`, `role="dialog"`/`aria-modal="true"` + caller-supplied
 * `aria-labelledby`/`aria-describedby`, configurable Escape/backdrop dismissal,
 * and the ref-counted scroll-lock above.
 *
 * Render it only while open (the caller returns `null` when closed); mounting is
 * the "open" signal, unmounting the "close" signal — matching how `ConfirmModal`
 * already gates on an `open` prop.
 */
export function Dialog({
  labelledBy,
  describedBy,
  onClose,
  dismissOnBackdrop = true,
  dismissOnEscape = true,
  className,
  children,
}: DialogProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  // Escape handling stays in useFocusTrap (unchanged behavior); disabled when the
  // caller opts out of Escape dismissal.
  useFocusTrap(panelRef, { onEscape: dismissOnEscape ? onClose : undefined });

  // Ref-counted scroll-lock for the open lifetime of this dialog.
  useEffect(() => {
    acquireScrollLock();
    return releaseScrollLock;
  }, []);

  // Enter transition: mount as `.scrim` (opacity 0 / panel offset) then flip to
  // `.scrim.open` on the next tick so the CSS transition runs. Reduced-motion
  // collapses --dur to 0, making this instant.
  const [entered, setEntered] = useState(false);
  useEffect(() => {
    setEntered(true);
  }, []);

  return createPortal(
    <div
      className={entered ? "scrim open" : "scrim"}
      onClick={(e) => {
        if (dismissOnBackdrop && e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        className={className ? `modal ${className}` : "modal"}
        // Clicks inside the panel must not bubble to the scrim's dismiss handler.
        onClick={(e) => e.stopPropagation()}
      >
        {children}
      </div>
    </div>,
    document.body,
  );
}
