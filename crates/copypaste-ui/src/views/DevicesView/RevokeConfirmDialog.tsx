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
      >
        <p id="revoke-modal-title">
          Revoke &ldquo;{name}&rdquo;
        </p>
        <p>
          Revoking removes this device from P2P. To also cut off cloud/relay
          sync, rotate the sync key — remaining devices must re-scan the
          pairing QR (or re-enter the new passphrase) to keep syncing. Rotate
          now?
        </p>

        <label>
          New sync passphrase (for rotation)
          {/* CopyPaste-5917.25: clarify this field is only used by Revoke & rotate,
              not by the plain Revoke only action. */}
          <span>— only used by "Revoke &amp; rotate"</span>
        </label>
        <input
          type="password"
          value={rotatePassphrase}
          onChange={(e) => onPassphraseChange(e.target.value)}
          placeholder="At least 8 characters"
          autoComplete="new-password"
          disabled={revokeBusy}
        />

        <div>
          <button
            onClick={onCancel}
            disabled={revokeBusy}
          >
            Cancel
          </button>
          <button
            onClick={() => onRevoke(fingerprint)}
            disabled={revokeBusy}
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
          >
            {/* bdac.83: aligned to Android label "Revoke & rotate key" for platform parity */}
            {revokeBusy ? "…" : "Revoke & rotate key"}
          </button>
        </div>
      </div>
    </div>
  );
}
