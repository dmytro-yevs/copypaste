import React, { useState } from "react";
import {
  formatWallTime,
  formatEpochSecs,
  type OwnDeviceInfo,
  type PairedDevice,
} from "../lib/ipc";
// i2sr (PG-40): hybrid relative/absolute formatter for last-sync timestamps.
import { formatSyncTime } from "../lib/time";

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
    // the Tailwind animate-pulse-ping span kept for test-compatibility.
    <span
      className={[
        "relative inline-flex shrink-0 items-center justify-center w-2 h-2",
        online ? "status-ping" : "",
      ].join(" ")}
    >
      {/* Expanding-ring pulse — only when online; respects prefers-reduced-motion */}
      {online && (
        <span
          aria-hidden="true"
          className="absolute inset-0 animate-pulse-ping rounded-full bg-ide-success/50 motion-reduce:animate-none"
        />
      )}
      <span
        title={title}
        aria-label={title}
        className={[
          "relative inline-block shrink-0 rounded-full",
          "w-2 h-2",
          // mztl: offline dot → danger red (matches SyncStatusChip and styleguide .dot-offline)
          online ? "bg-ide-success" : "bg-ide-danger",
        ].join(" ")}
        // success glow: soft halo matching the --success token (styleguide §presence)
        style={
          online
            ? {
                boxShadow:
                  "0 0 10px color-mix(in srgb, var(--success) 55%, transparent)",
              }
            : undefined
        }
      />
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
      <span className="text-[11px] text-ide-dim whitespace-nowrap">{label}</span>
      {/* tabular-nums keeps time/numeric values from causing layout shifts */}
      <span className="text-[11px] text-ide-faint break-all tabular-nums">{value}</span>
    </>
  );
}

// ---------------------------------------------------------------------------
// DeviceMetaGrid — wrapper that establishes the aligned two-column grid
// ---------------------------------------------------------------------------

export function DeviceMetaGrid({ children }: { children: React.ReactNode }) {
  return (
    <div
      className="mt-1.5 grid gap-x-3 gap-y-0.5 items-baseline"
      style={{ gridTemplateColumns: "auto 1fr" }}
    >
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

  // Truncate: first 8 chars + ellipsis + last 8 chars.
  const truncated =
    fingerprint.length > 20
      ? `${fingerprint.slice(0, 8)}…${fingerprint.slice(-8)}`
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
      <span className="text-[11px] text-ide-dim whitespace-nowrap">Fingerprint</span>
      {/* Clickable value — copies full fingerprint on click (Android parity).
          Uses a <button> so it is keyboard-accessible and screen-reader announced. */}
      <button
        type="button"
        data-testid="fingerprint-copy"
        title={`Copy full fingerprint: ${fingerprint}`}
        aria-label={`Fingerprint: ${truncated} — click to copy`}
        onClick={handleCopy}
        className="text-left text-[11px] text-ide-faint font-mono break-all tabular-nums hover:text-ide-accent cursor-pointer focus:outline-none"
      >
        {copied ? "Copied!" : truncated}
      </button>
    </>
  );
}

// ---------------------------------------------------------------------------
// ThisDeviceCard — rich identity block for the local device
// ---------------------------------------------------------------------------

export function ThisDeviceCard({ info }: { info: OwnDeviceInfo }) {
  return (
    <div className="px-3 py-2.5">
      {/* Name + online dot + "This Mac" badge */}
      <div className="flex flex-wrap items-center gap-1.5 mb-0.5">
        <StatusDot online={true} />
        <p className="truncate text-[13px] font-medium text-ide-text">
          {info.device_name ?? "This Device"}
        </p>
        {/* nmea: pill with hairline border matching accent tint; badge-float adds subtle levitate */}
        <span className="badge-float shrink-0 rounded-full border border-ide-accent/30 px-1.5 py-0.5 text-[10px] font-medium bg-ide-accent/14 text-ide-accent">
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

  // Transport chip: P2P when we have a local LAN address/ip; Cloud otherwise.
  // Defensive: no crash when address/local_ip are absent.
  const isP2p = !!(peer.local_ip || peer.address);
  const transportLabel = isP2p ? "P2P" : "Cloud";
  // sry7: transport chips → rounded-full pills with hairline border (nmea / 1hqt)
  // 1hqt: P2P uses sky token (not info) to match URL/IMAGE kind
  const transportClass = isP2p
    ? "text-ide-sky bg-ide-sky/14 border border-ide-sky/30 rounded-full"
    : "text-ide-accent bg-ide-accent/14 border border-ide-accent/30 rounded-full";

  return (
    // card-in: glass entrance fade-up (styleguide §device-card)
    // g4ze: changed from flex items-start justify-between to flex-col so the
    // action footer sits BELOW the metadata, not floated to the right.
    <div className="card-in px-3 pt-2.5 pb-2 hover:bg-ide-hover">
      {/* Content area — full width */}
      <div className="min-w-0">
        {/* Name + online dot + transport chip */}
        <div className="flex items-center gap-1.5">
          <StatusDot
            online={liveOnline !== undefined ? liveOnline : peer.online === true}
            lastSeenSecs={liveLastSeenSecs}
          />
          <p className="truncate text-[13px] font-medium text-ide-text">
            {peer.name || `Device ${peer.fingerprint.slice(0, 8)}`}
          </p>
          {/* nmea: transport chip — full-radius pill with hairline border; badge-float adds levitate */}
          <span
            className={[
              "badge-float shrink-0 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide",
              transportClass,
            ].join(" ")}
          >
            {transportLabel}
          </span>
        </div>

        {/* mgkr (NG-3): Verified trust badge — all persisted peers completed SAS
            confirmation before being added to the roster, so the badge is always
            "Verified". Uses success token tint + skin-aware radius (--skin-r-chip)
            so it adapts across classic / quiet / vapor skins without hardcoding. */}
        <span
          data-testid="trust-badge"
          style={{ borderRadius: "var(--skin-r-chip)" }}
          className="mt-1 inline-flex shrink-0 items-center gap-0.5 px-1.5 py-0.5 text-[10px] font-medium bg-ide-success/14 text-ide-success border border-ide-success/30"
        >
          {/* Tiny filled circle — visual indicator matching the styleguide presence dot */}
          <span className="inline-block w-1 h-1 rounded-full bg-ide-success" aria-hidden="true" />
          Verified
        </span>

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

        {/* Sync line: "Synced X ago" from last_sync_at */}
        {lastSyncStr && (
          <p className="mt-0.5 text-[11px] text-ide-faint tabular-nums">
            Synced {lastSyncStr}
          </p>
        )}

        {/* Revoked / error states — kept on their own line for visual weight */}
        {revokedAt !== null && (
          <p className="mt-0.5 text-[11px] text-ide-accent">
            Revoked · {formatWallTime(revokedAt)}
          </p>
        )}
        {rowError !== null && (
          <p className="mt-0.5 text-[11px] text-ide-danger">{rowError}</p>
        )}
      </div>

      {/* g4ze: Action footer — full-width row below metadata with hairline border-t.
           Both buttons are flex-1 equal width, matching Android's weight(1f) pattern.
           spec §7: both destructive actions use danger-tint fill (bg-ide-danger/15).
           W-C7: border-radius uses --skin-r-ctl (9px classic, 7px quiet, 12px vapor)
           via inline style so the skin axis drives the shape without hardcoding rounded-ide. */}
      <div className="mt-2 flex gap-1.5 border-t border-ide-border/20 pt-2">
        <button
          onClick={() => onUnpair(peer.fingerprint)}
          disabled={isPending}
          style={{ borderRadius: "var(--skin-r-ctl)" }}
          className="flex-1 bg-ide-danger/15 px-2.5 py-1 text-[12px] text-ide-danger hover:bg-ide-danger/25 disabled:cursor-not-allowed disabled:opacity-40"
        >
          {isPending ? "..." : "Unpair"}
        </button>
        <button
          onClick={() => onRevoke(peer.fingerprint)}
          disabled={isPending}
          style={{ borderRadius: "var(--skin-r-ctl)" }}
          className="flex-1 bg-ide-danger/15 px-2.5 py-1 text-[12px] text-ide-danger hover:bg-ide-danger/25 disabled:cursor-not-allowed disabled:opacity-40"
        >
          {isPending ? "..." : "Revoke"}
        </button>
      </div>
    </div>
  );
}
