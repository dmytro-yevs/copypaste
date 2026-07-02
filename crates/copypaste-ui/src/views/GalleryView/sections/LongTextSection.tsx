import { HistoryRow } from "../../HistoryView/HistoryRow";
import { PeerRow } from "../../../components/DeviceCard";
import { makeDevice, makeHistoryEntry } from "../../../lib/fixtures";
import { galleryRowDefaults } from "./rowDefaults";

const noop = () => {};

const LONG_TITLE =
  "This is a deliberately long single-line clipboard entry preview used to " +
  "verify row__title truncates with an ellipsis rather than wrapping or " +
  "overflowing its row, however many characters the daemon happens to send. " +
  "The quick brown fox jumps over the lazy dog, repeatedly, for good measure.";

const LONG_BUNDLE_ID =
  "com.some.extremely.long.reverse.dns.style.bundle.identifier.for.testing";
const LONG_DEVICE_NAME =
  "Dmytro's Extremely Long Custom Device Name Used For Testing Meta-Line Truncation";

const LONG_PEER_NAME =
  "A Very Long Peer Device Name That Should Truncate Gracefully In The Row Header";

const LONG_BANNER_MESSAGE =
  "This is a deliberately long banner message used to stress-test text " +
  "wrapping inside the .banner__x flex child, so the layout never overflows " +
  "its container or clips the dismiss action next to it, no matter how much " +
  "the daemon has to say. ".repeat(1);

// Task 6.11: "long-text and empty-state gallery coverage per component whose
// layout depends on content length (row titles, meta lines, device names,
// banner messages)" — the four examples named in the task, each shown as a
// long-text stress case next to a minimal/near-empty one.
export function LongTextSection() {
  return (
    <section id="gallery-long-text">
      <h2>Long-text &amp; minimal-content stress</h2>

      <h3>Row title</h3>
      <div className="list" role="listbox" aria-label="Row-title stress examples">
        <HistoryRow
          entry={makeHistoryEntry({ id: "gallery-long-title", preview: LONG_TITLE })}
          {...galleryRowDefaults}
        />
        <HistoryRow
          entry={makeHistoryEntry({ id: "gallery-min-title", preview: "x" })}
          {...galleryRowDefaults}
        />
      </div>

      <h3>Meta line</h3>
      <div className="list" role="listbox" aria-label="Meta-line stress examples">
        <HistoryRow
          entry={makeHistoryEntry({
            id: "gallery-long-meta",
            app_bundle_id: LONG_BUNDLE_ID,
            origin_device_name: LONG_DEVICE_NAME,
          })}
          {...galleryRowDefaults}
        />
        <HistoryRow
          entry={makeHistoryEntry({
            id: "gallery-min-meta",
            app_bundle_id: null,
            origin_device_name: null,
          })}
          {...galleryRowDefaults}
        />
      </div>

      <h3>Device name</h3>
      <div className="dev-list">
        <PeerRow
          peer={makeDevice({ fingerprint: "gallery-long-devname", name: LONG_PEER_NAME })}
          rowSt={undefined}
          onUnpair={noop}
          onRevoke={noop}
          liveLastSeenSecs={undefined}
          liveOnline={true}
        />
        {/* Empty name — DeviceCard falls back to "Device <fp-prefix>". */}
        <PeerRow
          peer={makeDevice({ fingerprint: "gallery-min-devname", name: "" })}
          rowSt={undefined}
          onUnpair={noop}
          onRevoke={noop}
          liveLastSeenSecs={undefined}
          liveOnline={false}
        />
      </div>

      <h3>Banner message</h3>
      <div className="gallery__col">
        <div className="banner banner--warn" role="alert">
          <span className="banner__x">{LONG_BANNER_MESSAGE}</span>
        </div>
        <div className="banner banner--info" role="status">
          <span className="banner__x">OK</span>
        </div>
      </div>
    </section>
  );
}
