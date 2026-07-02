// Extracted from DevicesView.tsx (CopyPaste-g06m.15).
// Cut/paste only — NO behavior changes.
//
// CopyPaste-g27b.11: wired to the redesign's .devrow row pattern. A discovered
// (unpaired) device has no expandable metadata — it renders as a static
// .devrow (no DisclosureHeader/.devrow__chev, no .devrow__foot) with a single
// "Pair" affordance, and its own not-pairable hint styled as .dev-hint.
import { Info, Link } from "lucide-react";
import { type DiscoveredDevice } from "../../lib/ipc";

/** One discovered (unpaired) LAN device row with a Pair button. */
export function DiscoveredRow({
  device,
  onPair,
  busy,
  index: _index = 0,
}: {
  device: DiscoveredDevice;
  onPair: (device: DiscoveredDevice) => void;
  busy: boolean;
  /** Row index for stagger timing (list-item-in). */
  index?: number;
}) {
  // Show all resolved IPs (comma-joined); fall back to a single address.
  const ips =
    device.ip_addrs.length > 0 ? device.ip_addrs.join(", ") : null;
  // v1 peers without a bootstrap port cannot do SAS pairing.
  const pairable = device.bport !== null;
  return (
    // list-item-in: staggered entrance; stagger delay = index × 60 ms (styleguide §list)
    <div className="devrow">
      <div className="devrow__head">
        <span className="devrow__name">
          {device.device_name || `Device ${device.device_id.slice(0, 8)}`}
        </span>
        {ips !== null && <span className="devrow__sum">{ips}</span>}
        {/* The one and only affordance on a discovered row — Pair. No
            Unpair/Revoke here (those only exist once a device is paired). */}
        <button
          type="button"
          className="btn btn--primary sm"
          onClick={() => onPair(device)}
          disabled={!pairable || busy}
          title={pairable ? undefined : "This device does not support secure pairing"}
        >
          <Link aria-hidden="true" />
          Pair
        </button>
      </div>
      {/* PG-43 / CopyPaste-3ese: Android parity — show a visible hint when the
          peer has no bootstrap port (bport=null) so the user knows why Pair is
          disabled, rather than silently greying it out (tooltip-only).
          Matches Android: "This device does not support secure pairing." */}
      {!pairable && (
        <p className="dev-hint">
          <Info aria-hidden="true" />
          This device does not support secure pairing
        </p>
      )}
    </div>
  );
}
