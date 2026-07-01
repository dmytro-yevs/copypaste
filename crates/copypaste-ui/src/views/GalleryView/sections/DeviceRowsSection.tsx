import { useEffect, useRef, type ReactNode } from "react";
import { ThisDeviceCard, PeerRow } from "../../../components/DeviceCard";
import { DiscoveredRow } from "../../DevicesView/DiscoveredRow";
import { makeDevice, makeDiscoveredDevice, makeOwnDeviceInfo } from "../../../lib/fixtures";

const noop = () => {};

/**
 * Gallery-only: clicks the first collapsed `.devrow__head` inside on mount so
 * a PeerRow example can be shown pre-expanded without user interaction. This
 * is a real DOM click on the real component's real disclosure button — it
 * does NOT add a prop/API to DeviceCard.tsx (keeps that component's surface
 * unchanged, per the DRY / no-speculative-API mandate).
 */
function AutoExpandOnMount({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const btn = ref.current?.querySelector<HTMLButtonElement>(
      '.devrow__head[aria-expanded="false"]',
    );
    btn?.click();
  }, []);
  return <div ref={ref}>{children}</div>;
}

// Task 6.7: "device row (own + peer, expanded + collapsed, one example per
// Decision 16 state)". ThisDeviceCard starts expanded by its own component
// default (satisfies "expanded"); a plain PeerRow starts collapsed by its own
// component default (satisfies "collapsed") — AutoExpandOnMount is used only
// where a state's footer/metadata needs to be visible without a click.
export function DeviceRowsSection() {
  return (
    <section id="gallery-device-row">
      <h2>Device row — own device + one example per Decision-16 state</h2>
      <div className="dev-list">
        {/* Own device — no destructive footer (Decision 16). Expanded by
            ThisDeviceCard's own default. */}
        <ThisDeviceCard info={makeOwnDeviceInfo()} />

        {/* Paired peer, online — Unpair/Revoke available. */}
        <AutoExpandOnMount>
          <PeerRow
            peer={makeDevice({
              fingerprint: "gallery-peer-online",
              name: "Online peer",
              online: true,
            })}
            rowSt={undefined}
            onUnpair={noop}
            onRevoke={noop}
            liveLastSeenSecs={undefined}
            liveOnline={true}
          />
        </AutoExpandOnMount>

        {/* Paired peer, offline — Unpair/Revoke availability is unchanged by
            online state (Decision 16). Left collapsed (component default). */}
        <PeerRow
          peer={makeDevice({
            fingerprint: "gallery-peer-offline",
            name: "Offline peer",
            online: false,
            last_seen_secs: 3_600,
          })}
          rowSt={undefined}
          onUnpair={noop}
          onRevoke={noop}
          liveLastSeenSecs={3_600}
          liveOnline={false}
        />

        {/* Discovered (unpaired) — only "Pair"; no trust relationship yet. */}
        <DiscoveredRow
          device={makeDiscoveredDevice({ device_id: "gallery-discovered" })}
          onPair={noop}
          busy={false}
        />

        {/* Action pending — disabled + spinner-eligible buttons. */}
        <AutoExpandOnMount>
          <PeerRow
            peer={makeDevice({ fingerprint: "gallery-peer-pending", name: "Pending action peer" })}
            rowSt={{ revokedAt: null, pending: true, error: null }}
            onUnpair={noop}
            onRevoke={noop}
            liveLastSeenSecs={undefined}
            liveOnline={true}
          />
        </AutoExpandOnMount>

        {/* Action failed — re-enabled + uniform inline error (Decision 16 NEW
            presentation). */}
        <AutoExpandOnMount>
          <PeerRow
            peer={makeDevice({ fingerprint: "gallery-peer-failed", name: "Failed action peer" })}
            rowSt={{ revokedAt: null, pending: false, error: "Network error — peer unreachable." }}
            onUnpair={noop}
            onRevoke={noop}
            liveLastSeenSecs={undefined}
            liveOnline={true}
          />
        </AutoExpandOnMount>
      </div>
    </section>
  );
}
