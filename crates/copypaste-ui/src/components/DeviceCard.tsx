import React, { useState } from "react";
import {
  formatWallTime,
  formatEpochSecs,
  type OwnDeviceInfo,
  type PairedDevice,
} from "../lib/ipc";
// i2sr (PG-40): hybrid relative/absolute formatter for last-sync timestamps.
import { formatSyncTime } from "../lib/time";
// bdac.14: shared button so danger-tint style comes from one source of truth.
import { ActionButton } from "./ActionButton";

// ---------------------------------------------------------------------------
// Shared device-card sub-components (CopyPaste-zxv2)
//
// Extracted from DevicesView.tsx so they can be reused across screens.
// All components keep the same visual spec as the originals.
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
  return (
    // relative wrapper so the pulse ring can be absolutely positioned behind the dot
    // status-ping: adds a CSS ::after expanding ring (styleguide §presence) in addition to
    // the animate-online-pulse span (MO-5 one-shot, crh3.18 — replaces animate-pulse-ping).
    <span>
      {/* Expanding-ring pulse — only when online; respects prefers-reduced-motion */}
      {online && <span aria-hidden="true" />}
      <span title={title} aria-label={title} />
    </span>
  );
}

// ---------------------------------------------------------------------------
// MetaRow — aligned two-column table row for device metadata
//
// Renders as a CSS-grid row so labels always line up vertically across all
// rows in the card. Hidden when value is absent/empty.
// ---------------------------------------------------------------------------

export function MetaRow({
  label,
  value,
}: {
  label: string;
  value: string | null | undefined;
}) {
  if (!value) return null;
  return (
    <>
      <span>{label}</span>
      <span>{value}</span>
    </>
  );
}

// ---------------------------------------------------------------------------
// DeviceMetaGrid — wrapper that establishes the aligned two-column grid
// ---------------------------------------------------------------------------

export function DeviceMetaGrid({ children }: { children: React.ReactNode }) {
  return (
    <div>
      {children}
    </div>
  );
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
    <>
      <span>Fingerprint</span>
      {/* Clickable value — copies full fingerprint on click (Android parity).
          Uses a <button> so it is keyboard-accessible and screen-reader announced. */}
      <button
        type="button"
        data-testid="fingerprint-copy"
        title={`Copy full fingerprint: ${fingerprint}`}
        aria-label={`Fingerprint: ${truncated} — click to copy`}
        onClick={handleCopy}
      >
        <span>{copied ? "Copied!" : truncated}</span>
      </button>
    </>
  );
}

// ---------------------------------------------------------------------------
// ThisDeviceCard — rich identity block for the local device
// ---------------------------------------------------------------------------

export function ThisDeviceCard({ info }: { info: OwnDeviceInfo }) {
  return (
    <div>
      {/* Name + online dot + "This Mac" badge */}
      <div>
        <StatusDot online={true} />
        <p>
          {info.device_name ?? "This Device"}
        </p>
        <span>
          This Mac
        </span>
      </div>

      {/* Aligned two-column metadata grid */}
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
// Layout change from CopyPaste-g4ze:
// Buttons moved from right-column float (items-start justify-between) to a
// full-width footer row BELOW the metadata, separated by a hairline border-t.
// Both buttons are flex-1 equal width — mirrors Android's action Row pattern.
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
  const isPending = rowSt?.pending ?? false;
  const revokedAt = rowSt?.revokedAt ?? null;
  const rowError = rowSt?.error ?? null;

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

  return (
    <div>
      {/* Content area — full width */}
      <div>
        {/* Name + online dot + transport chip */}
        <div>
          <StatusDot
            online={liveOnline !== undefined ? liveOnline : peer.online === true}
            lastSeenSecs={liveLastSeenSecs}
          />
          <p>
            {peer.name || `Device ${peer.fingerprint.slice(0, 8)}`}
          </p>
          <span>
            {transportLabel}
          </span>
        </div>

        {/* mgkr (NG-3) / CopyPaste-1jms.30: trust badge derived from peer.trust.
            "verified" → green Verified (SAS-confirmed peer).
            Any other value or absent → amber Unverified (matches Android trustLabel). */}
        {peer.trust === "verified" ? (
          <span data-testid="trust-badge">
            <span aria-hidden="true" />
            Verified
          </span>
        ) : (
          <span data-testid="trust-badge">
            <span aria-hidden="true" />
            Unverified
          </span>
        )}

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

        {/* Revoked / error states — kept on their own line for visual weight */}
        {revokedAt !== null && (
          <p>
            Revoked · {formatWallTime(revokedAt)}
          </p>
        )}
        {rowError !== null && (
          <p>{rowError}</p>
        )}
      </div>

      {/* g4ze: Action footer — full-width row below metadata with hairline border-t.
           Both buttons are flex-1 equal width, matching Android's weight(1f) pattern.
           bdac.14: use ActionButton(variant="danger") so the danger-tint style comes
           from a single source of truth in ActionButton.tsx (spec §7). */}
      <div>
        <ActionButton
          variant="danger"
          size="sm"
          onClick={() => onUnpair(peer.fingerprint)}
          disabled={isPending}
          pending={isPending}
          aria-label={`Unpair ${peer.name || peer.fingerprint.slice(0, 8)}`}
        >
          Unpair
        </ActionButton>
        <ActionButton
          variant="danger"
          size="sm"
          onClick={() => onRevoke(peer.fingerprint)}
          disabled={isPending}
          pending={isPending}
          aria-label={`Revoke ${peer.name || peer.fingerprint.slice(0, 8)}`}
        >
          Revoke
        </ActionButton>
      </div>
    </div>
  );
}
