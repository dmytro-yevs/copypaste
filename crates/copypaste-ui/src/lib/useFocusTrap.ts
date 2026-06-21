import { RefObject, useEffect, useRef } from "react";

const FOCUSABLE_SELECTORS = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(', ');

/**
 * Focus-trap hook for modal dialogs.
 *
 * On mount:
 *   - Saves the currently-focused element so it can be restored on unmount.
 *   - Focuses the first focusable child inside `ref.current` (or the container
 *     itself with tabIndex=-1 if no focusable children are found).
 *
 * While mounted:
 *   - Traps Tab / Shift+Tab so focus cycles within the container.
 *   - If `onEscape` is provided, calls it when the Escape key is pressed
 *     (A11Y-11 / CopyPaste-5917.30): callers don't each need to re-wire Escape.
 *
 * On unmount:
 *   - Restores focus to the element that was focused before the trap activated.
 */
export function useFocusTrap(
  ref: RefObject<HTMLElement | null>,
  { onEscape }: { onEscape?: () => void } = {}
) {
  // Capture the element that had focus when the modal opened.
  const previousFocusRef = useRef<Element | null>(null);
  // Keep onEscape in a ref so the keydown handler always sees the latest value
  // without needing to re-register the listener on every render.
  const onEscapeRef = useRef(onEscape);
  onEscapeRef.current = onEscape;

  useEffect(() => {
    previousFocusRef.current = document.activeElement;

    const container = ref.current;
    if (!container) return;

    // Focus the first focusable descendant; fall back to the container.
    const focusable = Array.from(
      container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTORS)
    );
    if (focusable.length > 0) {
      focusable[0].focus();
    } else {
      container.setAttribute('tabindex', '-1');
      container.focus();
    }

    function handleKeyDown(e: KeyboardEvent) {
      // Escape — delegate to caller if a handler was provided (A11Y-11).
      if (e.key === 'Escape') {
        if (onEscapeRef.current) {
          e.preventDefault();
          onEscapeRef.current();
        }
        return;
      }

      if (e.key !== 'Tab') return;
      const focusableEls = Array.from(
        container!.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTORS)
      );
      if (focusableEls.length === 0) return;
      const first = focusableEls[0];
      const last = focusableEls[focusableEls.length - 1];

      if (e.shiftKey) {
        // Shift+Tab: wrap backwards from first → last
        if (document.activeElement === first) {
          e.preventDefault();
          last.focus();
        }
      } else {
        // Tab: wrap forwards from last → first
        if (document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    }

    container.addEventListener('keydown', handleKeyDown);

    return () => {
      container.removeEventListener('keydown', handleKeyDown);
      // Restore focus to the element that was active before the modal opened.
      const prev = previousFocusRef.current;
      if (prev && typeof (prev as HTMLElement).focus === 'function') {
        (prev as HTMLElement).focus();
      }
    };
    // ref.current is stable for the lifetime of the modal; deps array is correct.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
