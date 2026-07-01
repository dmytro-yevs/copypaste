import { useRef } from "react";
import ReactDOM from "react-dom";
import { useFocusTrap } from "../lib/useFocusTrap";

/**
 * Shared confirmation modal.
 *
 * Renders via a portal into document.body so it layers above all view shells.
 * Focus is trapped inside (useFocusTrap) and restored when the modal closes.
 *
 * Props:
 *  - title      — heading shown in bold at the top.
 *  - body       — explanatory text (may be a ReactNode for rich content).
 *  - confirmLabel — label for the destructive confirm button (default "Confirm").
 *  - cancelLabel  — label for the cancel button (default "Cancel").
 *  - danger     — when true (default) the confirm button uses ide-danger styling.
 *  - busy       — when true both buttons are disabled and confirm shows "…".
 *  - onConfirm  — called when the confirm button is clicked.
 *  - onCancel   — called when Cancel is clicked or the backdrop is clicked.
 */
export interface ConfirmModalProps {
  title: string;
  body: React.ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

function ConfirmModalInner({
  title,
  body,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  busy = false,
  onConfirm,
  onCancel,
}: ConfirmModalProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  useFocusTrap(dialogRef);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="confirm-modal-title"
      // Clicking the backdrop dismisses the modal.
      onClick={(e) => { if (e.target === e.currentTarget) onCancel(); }}
      // Escape key also cancels.
      onKeyDown={(e) => { if (e.key === "Escape") { e.preventDefault(); onCancel(); } }}
    >
      <div ref={dialogRef}>
        <p id="confirm-modal-title">
          {title}
        </p>
        <div>
          {body}
        </div>
        <div>
          <button type="button" onClick={onCancel} disabled={busy}>
            {cancelLabel}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={busy}
            data-testid="confirm-modal-confirm-btn"
          >
            {busy ? "…" : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * Mounted via a React portal so it renders outside the view shell DOM tree.
 * Pass `open={false}` to unmount; pass `open={true}` to render the modal.
 * The modal plays modal-scrim-enter / modal-card-enter on mount.
 */
export function ConfirmModal(props: ConfirmModalProps & { open: boolean }) {
  const { open, ...rest } = props;
  if (!open) return null;
  // Use portal so the modal overlays all view shells unconditionally.
  return ReactDOM.createPortal(<ConfirmModalInner {...rest} />, document.body);
}
