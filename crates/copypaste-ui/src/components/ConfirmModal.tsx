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
  danger = true,
  busy = false,
  onConfirm,
  onCancel,
}: ConfirmModalProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  useFocusTrap(dialogRef);

  return (
    <div
      // modal-scrim-enter: approved motion entrance for modal backdrop (§MO-1).
      className="modal-scrim-enter fixed inset-0 z-[9998] flex items-center justify-center p-6"
      style={{ background: "var(--ide-scrim)" }}
      role="dialog"
      aria-modal="true"
      aria-labelledby="confirm-modal-title"
      // Clicking the backdrop dismisses the modal.
      onClick={(e) => { if (e.target === e.currentTarget) onCancel(); }}
      // Escape key also cancels.
      onKeyDown={(e) => { if (e.key === "Escape") { e.preventDefault(); onCancel(); } }}
    >
      {/* surface-glass-strong = floating frosted-glass material.
          modal-card-enter: approved motion entrance for modal card (§MO-1). */}
      <div
        ref={dialogRef}
        className="modal-card-enter surface-glass-strong w-full max-w-sm p-5"
        style={{ borderRadius: "var(--r-card)", boxShadow: "var(--sh3)" }}
      >
        <p id="confirm-modal-title" className="mb-2 text-[13px] font-semibold text-ide-text">
          {title}
        </p>
        <div className="mb-4 text-[12px] leading-relaxed text-ide-dim">
          {body}
        </div>
        <div className="flex items-center justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            disabled={busy}
            className="border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-dim hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
            style={{ borderRadius: "var(--r-ctl)" }}
          >
            {cancelLabel}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={busy}
            data-testid="confirm-modal-confirm-btn"
            className={[
              "px-3 py-1.5 text-[13px] font-medium disabled:cursor-not-allowed disabled:opacity-40",
              danger
                ? "bg-ide-danger text-white hover:bg-ide-danger/85"
                : "border border-ide-border bg-ide-elevated text-ide-text hover:bg-ide-hover",
            ].join(" ")}
            style={{ borderRadius: "var(--r-ctl)" }}
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
