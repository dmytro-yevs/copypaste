import { Dialog } from "../lib/dialog/Dialog";

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

/**
 * Composes the shared `Dialog` primitive (portal, role/aria-modal, focus trap,
 * Escape + backdrop dismissal, focus restoration, ref-counted scroll-lock).
 * Pass `open={false}` to unmount; `open={true}` to render. Behavior-preserving
 * refactor of the previous inline portal+focus-trap implementation (task 2.8).
 */
export function ConfirmModal({
  open,
  title,
  body,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  danger = true,
  busy = false,
  onConfirm,
  onCancel,
}: ConfirmModalProps & { open: boolean }) {
  if (!open) return null;
  return (
    <Dialog
      labelledBy="confirm-modal-title"
      describedBy="confirm-modal-body"
      onClose={onCancel}
    >
      <p id="confirm-modal-title" className="modal__t">
        {title}
      </p>
      <div id="confirm-modal-body" className="modal__s">
        {body}
      </div>
      <div className="modal__act">
        <button type="button" className="btn btn--ghost" onClick={onCancel} disabled={busy}>
          {cancelLabel}
        </button>
        <button
          type="button"
          className={danger ? "btn btn--danger" : "btn btn--primary"}
          onClick={onConfirm}
          disabled={busy}
          data-testid="confirm-modal-confirm-btn"
        >
          {busy ? "…" : confirmLabel}
        </button>
      </div>
    </Dialog>
  );
}
