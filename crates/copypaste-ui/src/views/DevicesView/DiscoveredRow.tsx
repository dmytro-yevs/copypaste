// Extracted from DevicesView.tsx (CopyPaste-g06m.15).
// Cut/paste only — NO behavior changes.
import { MetaRow } from "../../components/DeviceCard";
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
    <div>
      <div>
        <div>
          <p>
            {device.device_name || `Device ${device.device_id.slice(0, 8)}`}
          </p>
          <MetaRow label="Addresses" value={ips} />
          {/* PG-43 / CopyPaste-3ese: Android parity — show a visible hint when the
              peer has no bootstrap port (bport=null) so the user knows why Pair is
              disabled, rather than silently greying it out (tooltip-only).
              Matches Android: "This device does not support secure pairing." */}
          {!pairable && (
            <p>
              This device does not support secure pairing
            </p>
          )}
        </div>
        <button
          onClick={() => onPair(device)}
          disabled={!pairable || busy}
          title={pairable ? undefined : "This device does not support secure pairing"}
        >
          Pair
        </button>
      </div>
    </div>
  );
}
