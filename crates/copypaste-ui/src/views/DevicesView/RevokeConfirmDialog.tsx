// Extracted from DevicesView.tsx (CopyPaste-g06m.15).
// Cut/paste only — NO behavior changes.
//
// RevokeConfirmDialog — a small wrapper that applies a focus-trap to the
// revoke-device confirmation dialog. Extracted from DevicesView's inline JSX
// so the useFocusTrap hook can run unconditionally (hooks must not be called
// conditionally; the dialog is conditionally *rendered* by DevicesView).
import { useRef } from "react";
import { useFocusTrap } from "../../lib/useFocusTrap";

export function RevokeConfirmDialog({
  name,
  fingerprint,
  rotatePassphrase,
  revokeBusy,
  onPassphraseChange,
  onCancel,
  onRevoke,
  onRevokeAndRotate,
}: {
  name: string;
  fingerprint: string;
  rotatePassphrase: string;
  revokeBusy: boolean;
  onPassphraseChange: (v: string) => void;
  onCancel: () => void;
  onRevoke: (fp: string) => void;
  onRevokeAndRotate: (fp: string) => void;
}) {
  const dialogRef = useRef<HTMLDivElement>(null);
  // A11Y-4 / CopyPaste-5917.9: Escape dismisses the revoke dialog.
  // A11Y-11 / CopyPaste-5917.30: routed through useFocusTrap to avoid a separate listener.
  useFocusTrap(dialogRef, { onEscape: onCancel });

  return (
    <div
      className="modal-scrim-enter fixed inset-0 z-50 flex items-center justify-center p-6"
      style={{ background: "var(--ide-scrim)" }}
      role="dialog"
      aria-modal="true"
      aria-labelledby="revoke-modal-title"
      // A11Y-4 / CopyPaste-5917.9: Escape + backdrop click cancel the dialog.
      onClick={(e) => { if (e.target === e.currentTarget) onCancel(); }}
      onKeyDown={(e) => { if (e.key === "Escape") { e.preventDefault(); onCancel(); } }}
    >
      {/* surface-glass-strong = floating frosted-glass revoke dialog.
          modal-card-enter: approved motion entrance (§MO-1). */}
      <div
        ref={dialogRef}
        className="modal-card-enter surface-glass-strong w-full max-w-sm p-5"
        style={{ borderRadius: "var(--r-card)", boxShadow: "var(--sh3)" }}
      >
        <p id="revoke-modal-title" className="mb-1 text-[13px] font-medium text-ide-text">
          Revoke &ldquo;{name}&rdquo;
        </p>
        <p className="mb-3 text-[12px] leading-relaxed text-ide-dim">
          Revoking removes this device from P2P. To also cut off cloud/relay
          sync, rotate the sync key — remaining devices must re-scan the
          pairing QR (or re-enter the new passphrase) to keep syncing. Rotate
          now?
        </p>

        <label className="mb-1 block text-[11px] font-medium text-ide-faint">
          New sync passphrase (for rotation)
          {/* CopyPaste-5917.25: clarify this field is only used by Revoke & rotate,
              not by the plain Revoke only action. */}
          <span className="ml-1.5 font-normal text-ide-faint/70">— only used by "Revoke &amp; rotate"</span>
        </label>
        <input
          type="password"
          value={rotatePassphrase}
          onChange={(e) => onPassphraseChange(e.target.value)}
          placeholder="At least 8 characters"
          autoComplete="new-password"
          disabled={revokeBusy}
          className="mb-3 w-full border border-ide-border bg-ide-panel/60 px-2.5 py-1.5 text-[12px] text-ide-text placeholder:text-ide-faint focus:border-ide-accent/60 focus:outline-none disabled:opacity-40"
          style={{ borderRadius: "var(--r-ctl)" }}
        />

        <div className="flex items-center justify-end gap-2">
          <button
            onClick={onCancel}
            disabled={revokeBusy}
            className="border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-dim hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
            style={{ borderRadius: "var(--r-ctl)" }}
          >
            Cancel
          </button>
          <button
            onClick={() => onRevoke(fingerprint)}
            disabled={revokeBusy}
            className="border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-danger hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
            style={{ borderRadius: "var(--r-ctl)" }}
          >
            Revoke only
          </button>
          <button
            onClick={() => onRevokeAndRotate(fingerprint)}
            disabled={revokeBusy || rotatePassphrase.length < 8}
            title={
              rotatePassphrase.length < 8
                ? "Enter a new passphrase (min 8 chars) to rotate"
                : undefined
            }
            // wv57: aria-label is always set so screen readers can identify the
            // action even when the visible text is replaced by "..." when busy.
            aria-label="Revoke and rotate sync key"
            // puf4: solid-danger variant for primary destructive action (Revoke & rotate)
            className="bg-ide-danger px-3 py-1.5 text-[12px] font-medium text-white hover:bg-ide-danger/85 disabled:cursor-not-allowed disabled:opacity-40"
            style={{ borderRadius: "var(--r-ctl)" }}
          >
            {/* bdac.83: aligned to Android label "Revoke & rotate key" for platform parity */}
            {revokeBusy ? "…" : "Revoke & rotate key"}
          </button>
        </div>
      </div>
    </div>
  );
}
