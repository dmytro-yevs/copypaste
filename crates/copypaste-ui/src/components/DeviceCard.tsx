import React, { useState } from "react";
import { ChevronRight, ShieldOff, Unlink } from "lucide-react";
import {
  formatEpochSecs,
  type OwnDeviceInfo,
  type PairedDevice,
} from "../lib/ipc";
// i2sr (PG-40): hybrid relative/absolute formatter for last-sync timestamps.
import { formatSyncTime } from "../lib/time";
// CopyPaste-g27b.11: typed disclosure-header primitive drives the expandable
// device-row header (aria-expanded/aria-controls) — never a raw .btn.
import { DisclosureHeader } from "../lib/a11y/DisclosureHeader";

// ---------------------------------------------------------------------------
// Shared device-card sub-components (CopyPaste-zxv2)
//
// Extracted from DevicesView.tsx so they can be reused across screens.
// All components keep the same visual spec as the originals.
//
// CopyPaste-g27b.11: wired to the redesign's .devrow disclosure-list pattern
// (patterns.css). Presentation only — no IPC calls, handlers, pending flags,
// or action semantics changed.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// formatLastSeen — helper for StatusDot tooltip
// ---------------------------------------------------------------------------

function formatLastSeen(secs: number | undefined): string {
  if (secs === undefined || secs < 0) return "never";
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`;
  return `${Math.floor(secs / 86400)}d ago`;
}

// ---------------------------------------------------------------------------
// StatusDot — small coloured circle indicating online/offline presence
// ---------------------------------------------------------------------------

export function StatusDot({
  online,
  lastSeenSecs,
}: {
  online: boolean;
  lastSeenSecs?: number;
}) {
  const title = online
    ? "Online"
    : `Offline · last seen ${formatLastSeen(lastSeenSecs)}`;
  // .dot-stat carries its own CSS-only pulse (box-shadow keyframes) when
  // online, so no separate ping/ring child element is needed here.
  return (
    <span
      // g27b.28: role=img makes aria-label a permitted attribute (axe
      // aria-prohibited-attr) — this coloured dot conveys online/offline status
      // graphically, so it is semantically an image with a text alternative.
      role="img"
      className={online ? "dot-stat" : "dot-stat off"}
      title={title}
      aria-label={title}
    />
  );
}

// ---------------------------------------------------------------------------
// MetaRow — tap-to-copy device metadata field, rendered as a .cfield button
//
// Renders as one .cfield grid cell (label + value) inside a .cfields grid.
// Hidden when value is absent/empty. Clicking copies the value to the system
// clipboard, matching the FingerprintRow tap-to-copy affordance below (the
// .cfield/.cfield__k/.cfield__v/.copied contract is shared by all fields).
// ---------------------------------------------------------------------------

export function MetaRow({
  label,
  value,
}: {
  label: string;
  value: string | null | undefined;
}) {
  const [copied, setCopied] = useState(false);

  if (!value) return null;

  const handleCopy = () => {
    void navigator.clipboard.writeText(value).then(() => {
      setCopied(true);
      // Reset the "Copied!" feedback after 1.5 s.
      setTimeout(() => setCopied(false), 1500);
    });
  };

  return (
    <button
      type="button"
      className={copied ? "cfield copied" : "cfield"}
      onClick={handleCopy}
      aria-label={`${label}: ${value} — click to copy`}
    >
      <span className="cfield__k">{label}</span>
      <span className="cfield__v">{copied ? "Copied!" : value}</span>
    </button>
  );
}

// ---------------------------------------------------------------------------
// DeviceMetaGrid — wrapper that establishes the .cfields grid
// ---------------------------------------------------------------------------

export function DeviceMetaGrid({ children }: { children: React.ReactNode }) {
  return <div className="cfields">{children}</div>;
}

// ---------------------------------------------------------------------------
// FingerprintRow — truncated security fingerprint with tap-to-copy (cg2h)
//
// PG-9 spec: show first 8 + "…" + last 8 chars of the 64-char hex SHA-256
// fingerprint, matching the Android style. The full value is never displayed
// to avoid truncation at the CSS level — we truncate explicitly at the data
// level. Clicking copies the full fingerprint to the system clipboard.
// ---------------------------------------------------------------------------

function FingerprintRow({ fingerprint }: { fingerprint: string | null }) {
  const [copied, setCopied] = useState(false);

  if (!fingerprint) return null;

  // bdac.52: PARITY-SPEC §7 canonical format = first 16 chars + "…" + last 8 chars
  // (matches Android DevicesActivity.kt: fp.take(16) + "…" + fp.takeLast(8)).
  // Previous macOS format was 8+8 (16 chars total); canonical is 16+8 (24 chars).
  const truncated =
    fingerprint.length > 24
      ? `${fingerprint.slice(0, 16)}…${fingerprint.slice(-8)}`
      : fingerprint;

  const handleCopy = () => {
    void navigator.clipboard.writeText(fingerprint).then(() => {
      setCopied(true);
      // Reset the "Copied!" feedback after 1.5 s.
      setTimeout(() => setCopied(false), 1500);
    });
  };

  return (
    // Clickable value — copies full fingerprint on click (Android parity).
    // Uses a <button> so it is keyboard-accessible and screen-reader announced.
    <button
      type="button"
      className={copied ? "cfield copied" : "cfield"}
      data-testid="fingerprint-copy"
      title={`Copy full fingerprint: ${fingerprint}`}
      aria-label={`Fingerprint: ${truncated} — click to copy`}
      onClick={handleCopy}
    >
      <span className="cfield__k">Fingerprint</span>
      <span className="cfield__v">{copied ? "Copied!" : truncated}</span>
    </button>
  );
}

// ---------------------------------------------------------------------------
// ThisDeviceCard — rich identity block for the local device
// ---------------------------------------------------------------------------

export function ThisDeviceCard({ info }: { info: OwnDeviceInfo }) {
  // Own device row starts expanded (matches the design reference's
  // `devrow this open`) — purely a local UI toggle, not tied to any IPC state.
  const [expanded, setExpanded] = useState(true);
  const bodyId = "devrow-body-own";
  const summary = [info.os_version, info.local_ip].filter(Boolean).join(" · ");

  return (
    <div className={expanded ? "devrow this open" : "devrow this"}>
      {/* Name + online dot + "This Mac" badge */}
      <DisclosureHeader
        expanded={expanded}
        controls={bodyId}
        onToggle={() => setExpanded((v) => !v)}
        className="devrow__head"
      >
        <StatusDot online={true} />
        <span className="devrow__name">{info.device_name ?? "This Device"}</span>
        <span className="tpill tpill--this">This Mac</span>
        {summary !== "" && <span className="devrow__sum">{summary}</span>}
        <ChevronRight aria-hidden="true" className="devrow__chev" />
      </DisclosureHeader>

      {/* Aligned two-column metadata grid */}
      <div className="devrow__body" id={bodyId}>
        <div>
          <DeviceMetaGrid>
            <MetaRow label="Model" value={info.device_model} />
            <MetaRow label="OS" value={info.os_version} />
            <MetaRow label="Version" value={info.app_version} />
            <MetaRow label="Local IP" value={info.local_ip} />
            <MetaRow label="Public IP" value={info.public_ip ?? undefined} />
            {/* wb6s / cg2h: show own-device security fingerprint at parity with Android.
                Truncated to first8…last8 with tap-to-copy (PG-9 spec).
                Null when P2P is disabled (no cert generated). */}
            <FingerprintRow fingerprint={info.fingerprint} />
          </DeviceMetaGrid>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// DeviceRowState — per-row action state (shared with DevicesView)
// ---------------------------------------------------------------------------

export interface DeviceRowState {
  revokedAt: number | null;
  pending: boolean;
  error: string | null;
}

// ---------------------------------------------------------------------------
// extractIp — helper: extract just the IP part from a "host:port" string
// ---------------------------------------------------------------------------

export function extractIp(address: string | null | undefined): string | null {
  if (!address) return null;
  // IPv6 addresses look like [::1]:4242; IPv4 like 192.168.1.2:4242
  const v6 = address.match(/^\[(.+)\]:\d+$/);
  if (v6) return v6[1];
  const colon = address.lastIndexOf(":");
  if (colon > 0) return address.slice(0, colon);
  return address;
}

// ---------------------------------------------------------------------------
// PeerRow — one paired device entry (CopyPaste-zxv2 + CopyPaste-g4ze)
//
// CopyPaste-g27b.11: rendered as a .devrow disclosure item. The destructive
// footer (Unpair/Revoke, Decision 16) lives inside the expandable body, same
// as the design reference — collapsed by default so the actions require an
// intentional tap-to-expand first.
// ---------------------------------------------------------------------------

interface PeerRowProps {
  peer: PairedDevice;
  rowSt: DeviceRowState | undefined;
  onUnpair: (fp: string) => void;
  onRevoke: (fp: string) => void;
  /** A-4: live-adjusted last_seen_secs so the "Xm ago" label ticks every 1 s. */
  liveLastSeenSecs: number | undefined;
  /**
   * Live presence override from the peer-event broadcast channel.  When
   * `undefined`, falls back to `peer.online` from the last `list_peers` poll.
   * This lets the online dot react within ~1 s of a connect/disconnect without
   * waiting for the 10 s poll cycle.
   */
  liveOnline?: boolean;
}

export function PeerRow({
  peer,
  rowSt,
  onUnpair,
  onRevoke,
  liveLastSeenSecs,
  liveOnline,
}: PeerRowProps) {
  // Collapsed by default — only the local device row starts open.
  const [expanded, setExpanded] = useState(false);
  const isPending = rowSt?.pending ?? false;
  const revokedAt = rowSt?.revokedAt ?? null;
  const rowError = rowSt?.error ?? null;
  // CopyPaste-g27b.36b: once this row has been revoked (client-side, tracked
  // via rowSt.revokedAt from the revoke_peer/revoke_and_rotate response), the
  // row must stop presenting itself as an actively-paired device — no more
  // green online dot, "Verified" trust badge, or live Unpair/Revoke actions.
  const isRevoked = revokedAt !== null;

  // Prefer the peer's in-band advertised local_ip; fall back to parsing the
  // "host:port" P2P address field.
  const ip = peer.local_ip ?? extractIp(peer.address);

  // Format timestamps only when they have a real value.
  const pairedStr = (peer.added_at ?? 0) > 0 ? formatEpochSecs(peer.added_at) : null;
  // i2sr (PG-40): hybrid relative/absolute — relative when ≤24 h, absolute beyond.
  // last_sync_at is epoch seconds; formatSyncTime default unit is "secs".
  const lastSyncStr = formatSyncTime(peer.last_sync_at);

  // CopyPaste-1jms.32: 3-way transport chip.
  // When the daemon provides peer.transport, use it for an authoritative label.
  // Fallback: the legacy local_ip/address heuristic (P2P vs Cloud) for daemons
  // that predate the transport field (peer.transport absent or null).
  //
  // sry7: transport chips → rounded-full pills with hairline border (nmea / 1hqt)
  // 1hqt: P2P uses sky token (not info) to match URL/IMAGE kind
  // Relay uses warning/amber token (store-and-forward, not live).
  // Supabase uses accent/purple token (cloud backend).
  let transportLabel: string;
  if (peer.transport === "p2p") {
    transportLabel = "P2P";
  } else if (peer.transport === "relay") {
    transportLabel = "Relay";
  } else if (peer.transport === "supabase") {
    transportLabel = "Supabase";
  } else {
    // Fallback heuristic: local_ip or address present → likely P2P; else Cloud.
    const isP2pHeuristic = !!(peer.local_ip || peer.address);
    transportLabel = isP2pHeuristic ? "P2P" : "Cloud";
  }
  // CopyPaste-g27b.11: the redesign only ships .tpill--p2p/--cloud/--this
  // (patterns.css/primitives.css) — relay/Supabase keep their own text label
  // but visually share the "not P2P" cloud treatment. Never colour the row
  // itself by transport (Decision: transport shown ONLY via this chip).
  const tpillClass = transportLabel === "P2P" ? "tpill--p2p" : "tpill--cloud";

  // g27b.36b: a revoked device is never "online" regardless of the last
  // presence poll — it has just been cut off from P2P (and possibly cloud).
  const online = isRevoked ? false : liveOnline !== undefined ? liveOnline : peer.online === true;
  const summary = [peer.os_version, ip].filter(Boolean).join(" · ");
  const bodyId = `devrow-body-${peer.fingerprint}`;

  return (
    <div className={expanded ? "devrow open" : "devrow"}>
      {/* Name + online dot + trust badge + transport chip */}
      <DisclosureHeader
        expanded={expanded}
        controls={bodyId}
        onToggle={() => setExpanded((v) => !v)}
        className="devrow__head"
      >
        <StatusDot online={online} lastSeenSecs={liveLastSeenSecs} />
        <span className="devrow__name">
          {peer.name || `Device ${peer.fingerprint.slice(0, 8)}`}
        </span>
        {/* mgkr (NG-3) / CopyPaste-1jms.30: trust badge derived from peer.trust.
            "verified" → green Verified (SAS-confirmed peer).
            Any other value or absent → Unverified (matches Android trustLabel).
            g27b.36b: a revoked device no longer shows "Verified" (or
            "Unverified") — SAS trust is moot once P2P trust has been cut. */}
        {isRevoked ? (
          <span className="badge" data-testid="trust-badge">
            Revoked
          </span>
        ) : peer.trust === "verified" ? (
          <span className="badge badge--verified" data-testid="trust-badge">
            <span className="d" aria-hidden="true" />
            Verified
          </span>
        ) : (
          <span className="badge" data-testid="trust-badge">
            Unverified
          </span>
        )}
        <span className={`tpill ${tpillClass}`}>{transportLabel}</span>
        {summary !== "" && <span className="devrow__sum">{summary}</span>}
        <ChevronRight aria-hidden="true" className="devrow__chev" />
      </DisclosureHeader>

      <div className="devrow__body" id={bodyId}>
        <div>
          {/* Aligned two-column metadata grid — labels line up vertically */}
          <DeviceMetaGrid>
            <MetaRow label="Model" value={peer.model} />
            <MetaRow label="OS" value={peer.os_version} />
            <MetaRow label="Version" value={peer.app_version} />
            <MetaRow label="Local IP" value={ip} />
            <MetaRow label="Public IP" value={peer.public_ip} />
            <MetaRow label="Paired" value={pairedStr} />
            <MetaRow label="Last sync" value={lastSyncStr} />
            <MetaRow
              label="RTT"
              value={peer.latency_ms !== undefined ? `${peer.latency_ms} ms` : null}
            />
          </DeviceMetaGrid>

          {/* Revoked / error states — kept on their own line for visual weight.
              g27b.36b: revokedAt is Unix epoch SECONDS (as returned by
              revoke_peer/revoke_and_rotate) — formatEpochSecs (not
              formatWallTime, which expects milliseconds) is the correct
              formatter here; it also returns "—" for a falsy/0 timestamp, so
              a device revoked with an unknown time shows plain "Revoked"
              rather than a bogus epoch-1970 date. */}
          {isRevoked && (
            <p>{revokedAt ? `Revoked · ${formatEpochSecs(revokedAt)}` : "Revoked"}</p>
          )}
          {rowError !== null && (
            <p className="field-note field-note--err">{rowError}</p>
          )}

          {/* g4ze / g27b.19: Action footer — hairline border-t, right-aligned compact
               buttons (not full-width — Decision 16 superseded). Unpair and Revoke are
               two distinct-severity actions, not one destructive bar: Unpair is
               reversible (re-pair anytime) and uses .btn--warning; Revoke immediately
               breaks trust and keeps .btn--danger (Revoke & rotate stays inside
               RevokeConfirmDialog). Rendered as plain buttons (not ActionButton, which
               only knows primary/secondary/danger/danger-solid/ghost — no warning
               variant) so ActionButton.tsx (out of this slice's scope) stays untouched.
               g27b.36b: an already-revoked device has no live actions left to take —
               the footer is hidden entirely rather than showing disabled buttons. */}
          {!isRevoked && (
            <div className="devrow__foot">
              <button
                type="button"
                className="btn btn--warning sm"
                onClick={() => onUnpair(peer.fingerprint)}
                disabled={isPending}
                aria-label={`Unpair ${peer.name || peer.fingerprint.slice(0, 8)}`}
              >
                <Unlink aria-hidden="true" />
                {isPending ? "…" : "Unpair"}
              </button>
              <button
                type="button"
                className="btn btn--danger sm"
                onClick={() => onRevoke(peer.fingerprint)}
                disabled={isPending}
                aria-label={`Revoke ${peer.name || peer.fingerprint.slice(0, 8)}`}
              >
                <ShieldOff aria-hidden="true" />
                {isPending ? "…" : "Revoke"}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
