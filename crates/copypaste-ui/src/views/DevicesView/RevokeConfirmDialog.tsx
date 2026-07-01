// Extracted from DevicesView.tsx (CopyPaste-g06m.15).
// Cut/paste only — NO behavior changes.
//
// RevokeConfirmDialog — a small wrapper that applies a focus-trap to the
// revoke-device confirmation dialog. Extracted from DevicesView's inline JSX
// so the useFocusTrap hook can run unconditionally (hooks must not be called
// conditionally; the dialog is conditionally *rendered* by DevicesView).
import { Dialog } from "../../lib/dialog/Dialog";

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
  // A11Y-4/A11Y-11: Escape + backdrop dismissal and the focus trap now come from
  // the shared Dialog primitive (task 2.9). onClose=onCancel preserves behavior.
  return (
    <Dialog labelledBy="revoke-modal-title" onClose={onCancel}>
      <p id="revoke-modal-title" className="modal__t">
        Revoke &ldquo;{name}&rdquo;
      </p>
      <p className="modal__s">
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

      <div className="modal__act">
        <button
          type="button"
          className="btn btn--ghost"
          onClick={onCancel}
          disabled={revokeBusy}
        >
          Cancel
        </button>
        <button
          type="button"
          className="btn btn--danger"
          onClick={() => onRevoke(fingerprint)}
          disabled={revokeBusy}
        >
          Revoke only
        </button>
        <button
          type="button"
          className="btn btn--danger-solid"
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
        >
          {/* bdac.83: aligned to Android label "Revoke & rotate key" for platform parity */}
          {revokeBusy ? "…" : "Revoke & rotate key"}
        </button>
      </div>
    </Dialog>
  );
}
