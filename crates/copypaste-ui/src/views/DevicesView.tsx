import React, { useState, useEffect, useCallback, useRef } from "react";
import { Briefcase, Copy, RefreshCw, Zap, AlertCircle } from "lucide-react";
import {
  api,
  ipcErrorMessage,
  IpcError,
  formatWallTime,
  formatEpochSecs,
  pairingQrSvg,
  probeStatus,
  type DiscoveredDevice,
  type OwnDeviceInfo,
  type PairedDevice,
  type PairingQr,
  type PairSasStatus,
} from "../lib/ipc";
import { usePeerPresence } from "../lib/peerPresence";
import { useFocusTrap } from "../lib/useFocusTrap";
import { ViewShell } from "../components/ViewShell";
import { RestartDaemonButton } from "../components/RestartDaemonButton";
import { EmptyState } from "../components/EmptyState";

type QrState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; qr: PairingQr; generatedAt: number }
  | { status: "error"; message: string };

// QR blur is tracked independently of QR generation so regenerating does not
// accidentally clear the privacy blur (spec §10 / CopyPaste-v5a concern).
// Default: blurred (privacy-first). Cleared only when the user explicitly reveals.
type QrBlur = "blurred" | "revealed";

// Devices load outcomes. `degraded` (daemon up, DB unavailable) and `error`
// (some other failure) are split out from `offline` so a DB-degraded daemon is
// no longer mislabeled "Daemon not running."
type LoadState = "loading" | "offline" | "degraded" | "error" | "ready";

type OwnDeviceState =
  | { status: "loading" }
  | { status: "ready"; info: OwnDeviceInfo }
  | { status: "offline" };

interface DeviceRowState {
  revokedAt: number | null;
  pending: boolean;
  error: string | null;
}

// ---------------------------------------------------------------------------
// StatusDot — small coloured circle indicating online/offline presence
// ---------------------------------------------------------------------------

/** Format seconds-ago into a human-readable string for the offline tooltip. */
function formatLastSeen(secs: number | undefined): string {
  if (secs === undefined || secs < 0) return "never";
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`;
  return `${Math.floor(secs / 86400)}d ago`;
}

function StatusDot({
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
    <span className="relative inline-flex shrink-0 items-center justify-center w-2 h-2">
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

function MetaRow({ label, value }: { label: string; value: string | null | undefined }) {
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

function DeviceMetaGrid({ children }: { children: React.ReactNode }) {
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
// ThisDeviceCard — rich identity block for the local device
// ---------------------------------------------------------------------------

function ThisDeviceCard({
  info,
}: {
  info: OwnDeviceInfo;
}) {
  const [fpCopied, setFpCopied] = useState(false);
  const fpCopyTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    return () => {
      if (fpCopyTimer.current !== null) clearTimeout(fpCopyTimer.current);
    };
  }, []);

  const handleCopyFp = () => {
    if (!info.fingerprint) return;
    navigator.clipboard.writeText(info.fingerprint).then(
      () => {
        setFpCopied(true);
        if (fpCopyTimer.current !== null) clearTimeout(fpCopyTimer.current);
        fpCopyTimer.current = setTimeout(() => setFpCopied(false), 1500);
      },
      () => {
        // Clipboard denied — non-fatal; fingerprint is visible on screen.
      }
    );
  };

  return (
    <div className="px-3 py-2.5">
      {/* Name + online dot + "This Mac" badge */}
      <div className="flex flex-wrap items-center gap-1.5 mb-0.5">
        <StatusDot online={true} />
        <p className="truncate text-[13px] font-medium text-ide-text">
          {info.device_name ?? "This Device"}
        </p>
        {/* nmea: pill with hairline border matching accent tint */}
        <span className="shrink-0 rounded-full border border-ide-accent/30 px-1.5 py-0.5 text-[10px] font-medium bg-ide-accent/14 text-ide-accent">
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
      </DeviceMetaGrid>

      {/* Full fingerprint in mono + copy button */}
      {info.fingerprint && (
        <div className="mt-1.5 flex items-center gap-1.5">
          <span className="font-mono text-[10px] text-ide-faint break-all select-all">
            {info.fingerprint}
          </span>
          <button
            type="button"
            onClick={handleCopyFp}
            title="Copy fingerprint"
            aria-label="Copy fingerprint"
            className="shrink-0 rounded p-0.5 text-ide-faint hover:text-ide-dim hover:bg-ide-hover focus:outline-none focus-visible:ring-1 focus-visible:ring-ide-accent"
          >
            {fpCopied
              ? <span className="text-[10px] text-ide-success">✓</span>
              : <Copy size={12} strokeWidth={1.5} aria-hidden="true" />
            }
          </button>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// PeerRow — one paired device entry
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

function PeerRow({ peer, rowSt, onUnpair, onRevoke, liveLastSeenSecs, liveOnline }: PeerRowProps) {
  const isPending = rowSt?.pending ?? false;
  const revokedAt = rowSt?.revokedAt ?? null;
  const rowError = rowSt?.error ?? null;

  // Prefer the peer's in-band advertised local_ip; fall back to parsing the
  // "host:port" P2P address field.
  const ip = peer.local_ip ?? extractIp(peer.address);

  // Format timestamps only when they have a real value.
  const pairedStr = (peer.added_at ?? 0) > 0 ? formatEpochSecs(peer.added_at) : null;
  const lastSyncStr = formatEpochSecs(peer.last_sync_at);

  // Transport chip: P2P when we have a local LAN address/ip; Cloud otherwise.
  // Defensive: no crash when address/local_ip are absent.
  const isP2p = !!(peer.local_ip || peer.address);
  const transportLabel = isP2p ? "P2P" : "Cloud";
  // nmea: transport chips → rounded-full pills with hairline border
  // 1hqt: P2P uses sky token (not info) to match URL/IMAGE kind
  const transportClass = isP2p
    ? "text-ide-sky bg-ide-sky/14 border border-ide-sky/30 rounded-full"
    : "text-ide-accent bg-ide-accent/14 border border-ide-accent/30 rounded-full";

  // Truncated fingerprint: first 16 chars + ellipsis + last 8 chars.
  const fp = peer.fingerprint;
  const truncatedFp = fp.length > 24 ? fp.slice(0, 16) + "…" + fp.slice(-8) : fp;

  const [fpCopied, setFpCopied] = useState(false);
  const fpCopyTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    return () => {
      if (fpCopyTimer.current !== null) clearTimeout(fpCopyTimer.current);
    };
  }, []);

  const handleCopyFp = () => {
    navigator.clipboard.writeText(fp).then(
      () => {
        setFpCopied(true);
        if (fpCopyTimer.current !== null) clearTimeout(fpCopyTimer.current);
        fpCopyTimer.current = setTimeout(() => setFpCopied(false), 1500);
      },
      () => {
        // Clipboard denied — non-fatal.
      }
    );
  };

  return (
    <div className="px-3 py-2.5 hover:bg-ide-hover">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          {/* Name + online dot + transport chip */}
          <div className="flex items-center gap-1.5">
            <StatusDot
              online={liveOnline !== undefined ? liveOnline : peer.online === true}
              lastSeenSecs={liveLastSeenSecs}
            />
            <p className="truncate text-[13px] font-medium text-ide-text">
              {peer.name || `Device ${peer.fingerprint.slice(0, 8)}`}
            </p>
            {/* nmea: transport chip — full-radius pill with hairline border */}
            <span
              className={[
                "shrink-0 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide",
                transportClass,
              ].join(" ")}
            >
              {transportLabel}
            </span>
          </div>

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

          {/* Truncated fingerprint + hover-reveal copy button */}
          <div className="mt-1 flex items-center gap-1">
            <span className="font-mono text-[10px] text-ide-faint select-all">
              {truncatedFp}
            </span>
            <button
              type="button"
              onClick={handleCopyFp}
              title="Copy fingerprint"
              aria-label="Copy fingerprint"
              className="shrink-0 rounded p-0.5 text-ide-faint hover:text-ide-dim hover:bg-ide-hover focus:outline-none focus-visible:ring-1 focus-visible:ring-ide-accent"
            >
              {fpCopied
                ? <span className="text-[10px] text-ide-success">✓</span>
                : <Copy size={12} strokeWidth={1.5} aria-hidden="true" />
              }
            </button>
          </div>

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

        {/* Actions — both destructive actions always visible with the same
             danger-tint fill (spec §7 / CopyPaste-dpr consistency rule). */}
        <div className="flex shrink-0 items-center gap-1.5 pt-0.5">
          <button
            onClick={() => onUnpair(peer.fingerprint)}
            disabled={isPending}
            className="rounded-ide bg-ide-danger/15 px-2.5 py-1 text-[12px] text-ide-danger hover:bg-ide-danger/25 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {isPending ? "..." : "Unpair"}
          </button>
          <button
            onClick={() => onRevoke(peer.fingerprint)}
            disabled={isPending}
            className="rounded-ide bg-ide-danger/15 px-2.5 py-1 text-[12px] text-ide-danger hover:bg-ide-danger/25 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {isPending ? "..." : "Revoke"}
          </button>
        </div>
      </div>
    </div>
  );
}

/** Extract just the IP part from a "host:port" address string. */
function extractIp(address: string | null | undefined): string | null {
  if (!address) return null;
  // IPv6 addresses look like [::1]:4242; IPv4 like 192.168.1.2:4242
  const v6 = address.match(/^\[(.+)\]:\d+$/);
  if (v6) return v6[1];
  const colon = address.lastIndexOf(":");
  if (colon > 0) return address.slice(0, colon);
  return address;
}

// ---------------------------------------------------------------------------
// SAS pairing — discovery-initiated (LAN) pairing modal + discovered list
// ---------------------------------------------------------------------------

/** How often to poll `pair_get_sas` while the modal is open. */
const SAS_POLL_MS = 700;
/** How often to refresh the discovered-devices list while the view is open. */
const DISCOVERED_POLL_MS = 3000;

/**
 * Modal that drives the SAS pairing handshake for a single peer. Works for
 * BOTH the initiator (user clicked "Pair") and the responder (incoming pairing
 * detected by the background poll). On mount it polls `pair_get_sas`; closing
 * it (without a terminal Confirmed state) aborts the in-flight pairing.
 *
 * `device` is optional: present when the local user initiated (we already have
 * the DiscoveredDevice), absent on the responder side where peer identity comes
 * from the PairSasStatus fields returned by pair_get_sas.
 */
function SasPairingModal({
  device,
  displayName: displayNameProp,
  initialStatus,
  onClose,
  onPaired,
}: {
  /**
   * The discovered device being paired (initiator path). When absent, use
   * `displayName` instead (responder/incoming-pairing path).
   */
  device?: DiscoveredDevice | null;
  /**
   * Override display name — used on the responder side where no DiscoveredDevice
   * is available. Resolved as: peer_device_name ?? peer_name ?? "A device".
   */
  displayName?: string;
  /** Seed status for responder path — avoids a spurious "idle" flash on open. */
  initialStatus?: PairSasStatus;
  onClose: () => void;
  onPaired: () => void;
}) {
  const [status, setStatus] = useState<PairSasStatus>(
    initialStatus ?? { state: "initiating" }
  );
  const [error, setError] = useState<string | null>(null);
  const [confirmPending, setConfirmPending] = useState(false);
  const [sasCopied, setSasCopied] = useState(false);
  // True once the handshake ended without a local confirm (trailing `idle`
  // from the daemon's standing responder). Neutral terminal close state — not
  // an error, not a success. Distinct from the daemon's `aborted` wire state.
  const [ended, setEnded] = useState(false);
  // True once a terminal Confirmed has been observed — closing then must NOT
  // call pair_abort (the pairing already succeeded).
  const confirmedRef = useRef(false);
  // True once the user has locally accepted the SAS (clicked "Match"). Used to
  // disambiguate a trailing `idle` from the daemon's standing responder (see
  // the poll effect): local-accepted + idle ⇒ treat as success.
  const localAcceptedRef = useRef(false);
  // Component-lifetime unmount guard for async handlers (handleConfirm) so they
  // never setState after the modal closes. Distinct from the per-effect poll
  // cancellation flag, which scopes only its own loop.
  const unmountedRef = useRef(false);
  useEffect(() => {
    unmountedRef.current = false;
    return () => { unmountedRef.current = true; };
  }, []);
  // Focus trap — traps Tab/Shift+Tab inside the dialog panel and restores focus on close.
  const modalRef = useRef<HTMLDivElement>(null);
  useFocusTrap(modalRef);
  const copiedTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const terminal =
    ended ||
    status.state === "confirmed" ||
    status.state === "rejected" ||
    status.state === "aborted" ||
    status.state === "timed_out";

  // Poll pair_get_sas until a terminal state.
  //
  // The daemon's standing responder resets its state machine to `idle`
  // immediately after a terminal outcome, so a trailing `idle` observed AFTER
  // the handshake has been seen progressing (initiating/awaiting_sas) is itself
  // terminal — "pairing ended". We must NOT keep polling on it (that spins the
  // spinner forever). Interpretation: if the local user already accepted the
  // SAS, treat trailing idle as success (Paired + onPaired); otherwise show a
  // neutral "ended" close state.
  //
  // For the responder path, `initialStatus` seeds `awaiting_sas` so `sawActive`
  // starts true — a trailing idle after the user acts is treated as terminal.
  useEffect(() => {
    // Per-effect cancellation flag captured by this closure. Each effect run's
    // cleanup authoritatively stops only its own poll loop, so re-creating the
    // effect (e.g. on a stable-but-recreated dep) never orphans a prior loop.
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    // Whether this loop has ever seen a non-idle (active) handshake state.
    // Seed to true when the modal already starts in awaiting_sas (responder).
    let sawActive =
      initialStatus?.state === "initiating" ||
      initialStatus?.state === "awaiting_sas";

    const poll = async () => {
      try {
        const next = await api.pairGetSas();
        if (cancelled) return;

        if (next.state === "initiating" || next.state === "awaiting_sas") {
          sawActive = true;
          setStatus(next);
          timer = setTimeout(() => void poll(), SAS_POLL_MS);
          return;
        }

        if (next.state === "confirmed") {
          confirmedRef.current = true;
          setStatus(next);
          onPaired();
          return; // stop polling — terminal success
        }

        if (
          next.state === "rejected" ||
          next.state === "aborted" ||
          next.state === "timed_out"
        ) {
          setStatus(next);
          return; // stop polling — terminal failure
        }

        // next.state === "idle"
        if (sawActive) {
          // The handshake progressed then the responder reset to idle: the
          // pairing has ended. Resolve terminally — never re-poll on idle.
          if (confirmedRef.current || localAcceptedRef.current) {
            confirmedRef.current = true;
            setStatus({ state: "confirmed" });
            onPaired();
          } else {
            // Ended without a local confirm — neutral close state.
            setEnded(true);
          }
          return;
        }
        // Still idle before any active state was observed (e.g. the daemon
        // hasn't begun the handshake yet) — keep waiting.
        setStatus(next);
        timer = setTimeout(() => void poll(), SAS_POLL_MS);
      } catch (e) {
        if (cancelled) return;
        const msg = ipcErrorMessage(e, "Pairing status unavailable");
        setError(msg);
      }
    };

    void poll();
    return () => {
      cancelled = true;
      if (timer !== null) clearTimeout(timer);
    };
  }, [onPaired, initialStatus]);

  useEffect(() => {
    return () => {
      if (copiedTimer.current !== null) clearTimeout(copiedTimer.current);
    };
  }, []);

  // Close the modal. Aborts the pairing unless it already succeeded.
  const handleClose = useCallback(() => {
    if (!confirmedRef.current) {
      // Best-effort abort; ignore failure (modal is closing regardless).
      void api.pairAbort().catch(() => {});
    }
    onClose();
  }, [onClose]);

  const handleConfirm = useCallback(
    async (accept: boolean) => {
      setConfirmPending(true);
      setError(null);
      // Record the local accept up-front so a trailing `idle` from the daemon's
      // standing responder is interpreted as success even if it arrives before
      // a `confirmed` tick.
      if (accept) localAcceptedRef.current = true;
      try {
        await api.pairConfirmSas(accept);
        if (unmountedRef.current) return;
        if (!accept) {
          // User said it doesn't match — close immediately.
          onClose();
          return;
        }
        // On accept, keep polling; the next poll tick reflects confirmed/rejected.
      } catch (e) {
        // The accept never reached the daemon — undo the optimistic flag so a
        // later trailing idle isn't misread as success.
        localAcceptedRef.current = false;
        if (unmountedRef.current) return;
        const msg = ipcErrorMessage(e, "Failed to send decision");
        setError(msg);
      } finally {
        if (!unmountedRef.current) setConfirmPending(false);
      }
    },
    [onClose]
  );

  const handleCopySas = useCallback(() => {
    if (status.sas === undefined) return;
    const sas = status.sas;
    navigator.clipboard.writeText(sas).then(
      () => {
        setSasCopied(true);
        if (copiedTimer.current !== null) clearTimeout(copiedTimer.current);
        copiedTimer.current = setTimeout(() => setSasCopied(false), 1500);
      },
      () => {
        // Clipboard denied — non-fatal; the code is visible on screen.
      }
    );
  }, [status.sas]);

  // Resolve the display name for the modal title. Initiator: the full
  // DiscoveredDevice. Responder/incoming-pairing: an explicit displayName prop
  // (from the App-level incoming-pairing event) or the peer meta the daemon
  // surfaces in the SAS status payload.
  const peerName =
    displayNameProp ||
    device?.device_name ||
    status.peer_device_name ||
    status.peer_name ||
    (device ? `Device ${device.device_id.slice(0, 8)}` : "A device");

  const isResponder = status.role === "responder";

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-6"
      role="dialog"
      aria-modal="true"
      aria-labelledby="sas-modal-title"
    >
      {/* surface-glass-strong = floating frosted-glass pairing dialog. */}
      <div ref={modalRef} className="surface-glass-strong w-full max-w-sm rounded-ide-lg p-5 shadow-ide-lg">
        <p id="sas-modal-title" className="mb-1 text-[13px] font-medium text-ide-text">
          {isResponder ? `"${peerName}" wants to pair` : `Pair "${peerName}"`}
        </p>

        {/* Peer metadata (responder path, or when daemon provides it) */}
        {(status.peer_model || status.peer_os || status.peer_app_version || status.peer_ip) && (
          <div className="mt-1 mb-2 space-y-0.5">
            <MetaRow label="Model" value={status.peer_model} />
            <MetaRow label="OS" value={status.peer_os} />
            <MetaRow label="Version" value={status.peer_app_version} />
            <MetaRow label="IP" value={status.peer_ip} />
          </div>
        )}

        {/* Connecting / initiating */}
        {!ended && status.state === "initiating" && error === null && (
          <div className="flex items-center gap-2 py-4">
            <span className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-ide-faint border-t-ide-accent" />
            <p className="text-[12px] text-ide-dim">Connecting...</p>
          </div>
        )}

        {/* audit P2: pre-handshake placeholder. Before the daemon reports a
            recognised state (idle/empty/unknown — e.g. the responder modal opened
            from the incoming-pairing event before the SAS poll's first tick), the
            body was a blank box. Show a waiting spinner so it's never empty. */}
        {!ended &&
          error === null &&
          status.state !== "initiating" &&
          status.state !== "awaiting_sas" &&
          status.state !== "confirmed" &&
          status.state !== "rejected" &&
          status.state !== "aborted" &&
          status.state !== "timed_out" && (
            <div className="flex items-center gap-2 py-4">
              <span className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-ide-faint border-t-ide-accent" />
              <p className="text-[12px] text-ide-dim">Waiting for the other device…</p>
            </div>
          )}

        {/* Awaiting SAS — show the code prominently */}
        {!ended && status.state === "awaiting_sas" && status.sas !== undefined && (
          <div className="py-2">
            <p className="mb-2 text-[12px] text-ide-dim">
              Confirm this code matches the one shown on the other device.
            </p>
            <button
              type="button"
              onClick={handleCopySas}
              title={sasCopied ? "Copied!" : "Click to copy"}
              className="mx-auto block rounded-ide bg-ide-panel/60 px-4 py-3 font-mono text-[28px] font-semibold tracking-[0.3em] text-ide-text hover:bg-ide-hover focus:outline-none focus-visible:ring-2 focus-visible:ring-ide-accent"
            >
              {status.sas}
              {sasCopied && <span className="ml-2 text-[12px] text-ide-success">✓</span>}
            </button>
            {/* Peer metadata grid — rendered from whatever the daemon knows at
                SAS time (mDNS: name, IPs, fingerprint). All rows are optional;
                nothing renders when a field is absent (responder path). */}
            {(status.peer_device_name ??
              status.peer_ip_addrs?.length ??
              status.peer_fingerprint) && (
              <div className="mt-3 rounded-ide border border-ide-divider bg-ide-panel/40 px-3 py-2">
                <MetaRow label="Name" value={status.peer_device_name} />
                <MetaRow
                  label="Addresses"
                  value={status.peer_ip_addrs?.join(", ")}
                />
                <MetaRow label="Fingerprint" value={status.peer_fingerprint} />
              </div>
            )}
            <div className="mt-4 flex items-center justify-end gap-2">
              <button
                onClick={() => void handleConfirm(false)}
                disabled={confirmPending}
                className="rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-dim hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
              >
                Doesn't match
              </button>
              <button
                onClick={() => void handleConfirm(true)}
                disabled={confirmPending}
                className="rounded-ide bg-ide-accent px-3 py-1.5 text-[12px] font-medium text-white hover:bg-ide-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
              >
                {confirmPending ? "..." : "Match"}
              </button>
            </div>
          </div>
        )}

        {/* Waiting after the user accepted, for the peer to also accept */}
        {!ended && status.state === "awaiting_sas" && status.sas === undefined && (
          <div className="flex items-center gap-2 py-4">
            <span className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-ide-faint border-t-ide-accent" />
            <p className="text-[12px] text-ide-dim">Waiting for the other device...</p>
          </div>
        )}

        {/* Terminal success */}
        {status.state === "confirmed" && (
          <div className="py-3">
            <p className="text-[13px] font-medium text-ide-success">Paired ✓</p>
            <div className="mt-3 flex justify-end">
              <button
                onClick={onClose}
                className="rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-text hover:bg-ide-hover"
              >
                Close
              </button>
            </div>
          </div>
        )}

        {/* Terminal failure */}
        {(status.state === "rejected" ||
          status.state === "aborted" ||
          status.state === "timed_out") && (
          <div className="py-3">
            <p className="text-[13px] text-ide-danger">
              {status.state === "timed_out"
                ? "Pairing timed out."
                : status.state === "rejected"
                  ? "Pairing was rejected."
                  : "Pairing was cancelled."}
            </p>
            <div className="mt-3 flex justify-end">
              <button
                onClick={onClose}
                className="rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-text hover:bg-ide-hover"
              >
                Close
              </button>
            </div>
          </div>
        )}

        {/* Neutral terminal "ended" state — the daemon's standing responder
            reset to idle after the handshake without a local confirm. Not an
            error: the pairing simply ended (likely resolved on the other
            device). */}
        {ended && (
          <div className="py-3">
            <p className="text-[13px] text-ide-dim">
              Pairing ended — check the other device.
            </p>
            <div className="mt-3 flex justify-end">
              <button
                onClick={onClose}
                className="rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-text hover:bg-ide-hover"
              >
                Close
              </button>
            </div>
          </div>
        )}

        {/* Transient poll/confirm error (non-terminal) */}
        {error !== null && !terminal && (
          <p className="mt-2 text-[11px] text-ide-danger">{error}</p>
        )}

        {/* Cancel affordance for the non-terminal states (Connecting / awaiting) */}
        {!terminal && (
          <div className="mt-4 border-t border-ide-divider pt-3 text-right">
            <button
              onClick={handleClose}
              className="text-[11px] text-ide-faint hover:text-ide-dim"
            >
              Cancel
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

/** One discovered (unpaired) LAN device row with a Pair button. */
function DiscoveredRow({
  device,
  onPair,
  busy,
}: {
  device: DiscoveredDevice;
  onPair: (device: DiscoveredDevice) => void;
  busy: boolean;
}) {
  // Show all resolved IPs (comma-joined); fall back to a single address.
  const ips =
    device.ip_addrs.length > 0 ? device.ip_addrs.join(", ") : null;
  // v1 peers without a bootstrap port cannot do SAS pairing.
  const pairable = device.bport !== null;
  return (
    <div className="px-3 py-2.5 hover:bg-ide-hover">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] font-medium text-ide-text">
            {device.device_name || `Device ${device.device_id.slice(0, 8)}`}
          </p>
          <MetaRow label="Addresses" value={ips} />
          <MetaRow label="Fingerprint" value={device.device_id || null} />
        </div>
        <button
          onClick={() => onPair(device)}
          disabled={!pairable || busy}
          title={pairable ? undefined : "This device does not support secure pairing"}
          className="shrink-0 rounded-ide bg-ide-accent px-2.5 py-1 text-[12px] font-medium text-white hover:bg-ide-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
        >
          Pair
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// RevokeConfirmDialog — a small wrapper that applies a focus-trap to the
// revoke-device confirmation dialog. Extracted from DevicesView's inline JSX
// so the useFocusTrap hook can run unconditionally (hooks must not be called
// conditionally; the dialog is conditionally *rendered* by DevicesView).
// ---------------------------------------------------------------------------

function RevokeConfirmDialog({
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
  useFocusTrap(dialogRef);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-6"
      role="dialog"
      aria-modal="true"
      aria-labelledby="revoke-modal-title"
    >
      {/* surface-glass-strong = floating frosted-glass revoke dialog. */}
      <div ref={dialogRef} className="surface-glass-strong w-full max-w-sm rounded-ide-lg p-5 shadow-ide-lg">
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
        </label>
        <input
          type="password"
          value={rotatePassphrase}
          onChange={(e) => onPassphraseChange(e.target.value)}
          placeholder="At least 8 characters"
          autoComplete="new-password"
          disabled={revokeBusy}
          className="mb-3 w-full rounded-ide border border-ide-border bg-ide-panel/60 px-2.5 py-1.5 text-[12px] text-ide-text placeholder:text-ide-faint focus:border-ide-accent/60 focus:outline-none disabled:opacity-40"
        />

        <div className="flex items-center justify-end gap-2">
          <button
            onClick={onCancel}
            disabled={revokeBusy}
            className="rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-dim hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
          >
            Cancel
          </button>
          <button
            onClick={() => onRevoke(fingerprint)}
            disabled={revokeBusy}
            className="rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-danger hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
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
            // puf4: solid-danger variant for primary destructive action (Revoke & rotate)
            className="rounded-ide bg-ide-danger px-3 py-1.5 text-[12px] font-medium text-white hover:bg-ide-danger/85 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {revokeBusy ? "..." : "Revoke & rotate"}
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main view
// ---------------------------------------------------------------------------

// Pairing token TTL from the daemon (PAKE_SESSION_TTL = 120 s).
// We refresh 15 s before expiry to ensure a valid code is always on-screen.
const QR_TTL_SECS = 120;
const QR_REFRESH_MARGIN_SECS = 15;

// ---------------------------------------------------------------------------
// Main view
// ---------------------------------------------------------------------------

export function DevicesView({
  incomingPairing = null,
}: {
  /**
   * When the Tauri backend detects an inbound pairing request (responder side),
   * App.tsx passes the `pair_get_sas` payload here so the modal opens regardless
   * of which tab was active when the request arrived.
   */
  incomingPairing?: PairSasStatus | null;
} = {}) {
  // --- Live peer-presence from the global event-broadcast store ---
  // Updated ~every 1 s by `App.tsx`'s `startPeerPresencePolling()` loop.
  // Used as a high-frequency overlay on top of the 10 s `list_peers` poll so
  // online dots change within ~1 s of a real connect/disconnect.
  const presenceOnline = usePeerPresence((s) => s.online);

  // --- Own device info ---
  const [ownState, setOwnState] = useState<OwnDeviceState>({ status: "loading" });
  // Ref that always holds the latest own fingerprint so loadPeers (a useCallback)
  // can read the current value without closing over a stale ownState snapshot.
  const ownFpRef = useRef<string | null>(null);

  // A-4: 1 s clock tick so "last seen Xm ago" labels update live between
  // the 10 s loadPeers polls. Stored as epoch-seconds so PeerRow can compute
  // the elapsed offset without closing over a stale snapshot.
  const [nowSecs, setNowSecs] = useState(() => Math.floor(Date.now() / 1000));
  // Ref to the epoch-second timestamp when peers data was last fetched from
  // the daemon; used to compute live last_seen_secs = daemon_value + elapsed.
  const peersFetchedAtRef = useRef<number>(Math.floor(Date.now() / 1000));
  useEffect(() => {
    const id = setInterval(() => {
      setNowSecs(Math.floor(Date.now() / 1000));
    }, 1000);
    return () => { clearInterval(id); };
  }, []);

  // --- Paired peers ---
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [peers, setPeers] = useState<PairedDevice[]>([]);
  const [rowState, setRowState] = useState<Record<string, DeviceRowState>>({});
  const [revokeAllPending, setRevokeAllPending] = useState(false);
  const [revokeAllConfirm, setRevokeAllConfirm] = useState(false);
  // C-P0-4: the device a Revoke confirm dialog is open for (null = closed).
  const [revokePrompt, setRevokePrompt] = useState<{ fingerprint: string; name: string } | null>(
    null,
  );
  // New sync-key passphrase entered in the "Revoke & rotate" path.
  const [rotatePassphrase, setRotatePassphrase] = useState("");
  const [revokeBusy, setRevokeBusy] = useState(false);
  const [globalMsg, setGlobalMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const globalMsgTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // --- LAN discovery + SAS pairing ---
  const [discovered, setDiscovered] = useState<DiscoveredDevice[]>([]);
  // The device a SAS pairing modal is currently open for, or null.
  const [pairingDevice, setPairingDevice] = useState<DiscoveredDevice | null>(null);
  // Set true while pair_with_discovered is in flight (before the modal opens).
  const [pairStarting, setPairStarting] = useState(false);
  // Inline error shown beneath the discovered list (e.g. rate-limited).
  const [discoverError, setDiscoverError] = useState<string | null>(null);
  // HB-9: true while a manual mDNS rescan is in flight (Refresh button).
  const [rescanning, setRescanning] = useState(false);
  // --- Incoming (responder) pairing ---
  // When the backend emits "incoming-pairing" (App.tsx routes it here), we open
  // the SAS modal pre-seeded with the inbound status rather than waiting for the
  // user to be on the Devices tab.  We track it separately from `pairingDevice`
  // because the responder has no DiscoveredDevice — only the SAS status payload.
  const [responderPairing, setResponderPairing] = useState<PairSasStatus | null>(
    // Initialise from the prop so the modal opens immediately on first render
    // if App.tsx already has an in-flight inbound pairing when it mounts/remounts
    // DevicesView (e.g. because it just switched to the Devices tab).
    incomingPairing ?? null
  );

  // Keep responderPairing in sync when the prop changes (App.tsx may update it
  // after the component is already mounted).
  useEffect(() => {
    if (incomingPairing != null) {
      setResponderPairing(incomingPairing);
    }
  }, [incomingPairing]);

  // --- QR pairing ---
  const [qrState, setQrState] = useState<QrState>({ status: "idle" });
  // Countdown seconds remaining until the current QR expires (display only).
  const [qrSecsLeft, setQrSecsLeft] = useState<number | null>(null);
  // Ref so the auto-refresh timer can read the latest qrState without a
  // stale-closure problem — we write it in parallel with the React state.
  const qrStateRef = useRef<QrState>({ status: "idle" });
  // Inflight guard: prevents two concurrent generateQr calls (e.g. auto-refresh
  // tick racing a manual click) from both issuing a pairingQrSvg() request and
  // wasting single-use tokens. Unmount flag doubles as a cancelled guard.
  const qrInflightRef = useRef(false);
  const qrCancelledRef = useRef(false);

  // QR privacy blur — independent of QR generation so regenerating the QR code
  // does not accidentally clear the blur (CopyPaste-v5a). Default: blurred.
  const [qrBlur, setQrBlur] = useState<QrBlur>("blurred");

  const generateQr = useCallback(async () => {
    // Drop duplicate concurrent calls — only one generation runs at a time.
    if (qrInflightRef.current) return;
    qrInflightRef.current = true;
    setQrState({ status: "loading" });
    qrStateRef.current = { status: "loading" };
    setQrSecsLeft(null);
    try {
      const qr = await pairingQrSvg();
      // Don't update state if the component unmounted while we awaited.
      if (qrCancelledRef.current) return;
      const next: QrState = { status: "ready", qr, generatedAt: Date.now() };
      setQrState(next);
      qrStateRef.current = next;
      setQrSecsLeft(qr.expires_in_secs > 0 ? qr.expires_in_secs : QR_TTL_SECS);
    } catch (err) {
      if (qrCancelledRef.current) return;
      const message = err instanceof Error ? err.message : "Failed to generate pairing code";
      const next: QrState = { status: "error", message };
      setQrState(next);
      qrStateRef.current = next;
    } finally {
      qrInflightRef.current = false;
    }
  }, []);

  // Clicking the QR when blurred reveals it; when already revealed, regenerates.
  // This keeps reveal and regeneration as two distinct affordances (spec §10).
  const handleQrReveal = useCallback(() => {
    setQrBlur("revealed");
  }, []);

  const handleQrRegenerate = useCallback(() => {
    // Blur is kept as-is across regeneration — the user must reveal explicitly.
    void generateQr();
  }, [generateQr]);

  // --- Load own device info ---
  // A-2: extracted to a stable useCallback so it can be called both on mount
  // and on the 10 s polling interval (same cadence as loadPeers) so that
  // Public IP (resolved async via STUN after mount) and Local IP (may change
  // on network switch) stay fresh without requiring a manual refresh.
  const loadOwnInfo = useCallback(async () => {
    try {
      const info = await api.getOwnDeviceInfo();
      // Keep the ref in sync so loadPeers always reads the latest fingerprint
      // without closing over a stale ownState snapshot.
      ownFpRef.current = info.fingerprint ?? null;
      setOwnState({ status: "ready", info });
    } catch (err: unknown) {
      const code = err instanceof IpcError ? err.code : null;
      if (code === "daemon_offline") {
        setOwnState({ status: "offline" });
      } else {
        // Daemon is up but method may not exist on older daemon builds —
        // treat as offline so the UI still shows the fingerprint section.
        setOwnState({ status: "offline" });
      }
    }
  }, []);

  // Initial load on mount — sets "loading" first, then resolves via loadOwnInfo.
  useEffect(() => {
    setOwnState({ status: "loading" });
    void loadOwnInfo();
  }, [loadOwnInfo]);

  // Poll own-device info every 10 s (same interval as loadPeers) so Public IP
  // (resolved async via STUN) and Local IP (may change on network switch)
  // stay fresh without user interaction.
  const OWN_INFO_POLL_MS = 10_000;
  useEffect(() => {
    const id = setInterval(() => { void loadOwnInfo(); }, OWN_INFO_POLL_MS);
    return () => { clearInterval(id); };
  }, [loadOwnInfo]);

  // Auto-generate QR on mount, auto-refresh before expiry.
  //
  // Visibility-gated (mirrors Popup.tsx): the 1 s tick — and therefore the
  // ~every-105 s single-use-token regeneration — only runs while the window is
  // in the foreground. A backgrounded/hidden window would otherwise keep burning
  // fresh single-use pairing tokens that nobody is looking at. When the window
  // becomes visible again the tick resumes; if the on-screen token already
  // expired while hidden, the first tick (remaining <= margin) regenerates it.
  useEffect(() => {
    qrCancelledRef.current = false;
    void generateQr();

    let interval: ReturnType<typeof setInterval> | null = null;

    const tick = () => {
      const current = qrStateRef.current;
      if (current.status !== "ready") return;

      const elapsedSecs = (Date.now() - current.generatedAt) / 1000;
      const ttl = current.qr.expires_in_secs > 0 ? current.qr.expires_in_secs : QR_TTL_SECS;
      const remaining = Math.max(0, Math.round(ttl - elapsedSecs));
      setQrSecsLeft(remaining);

      // Refresh QR_REFRESH_MARGIN_SECS before expiry so the user always has
      // a scannable code — single-use tokens expire after QR_TTL_SECS.
      if (remaining <= QR_REFRESH_MARGIN_SECS) {
        void generateQr();
      }
    };

    const start = () => {
      if (interval !== null) return;
      // Tick every second: update the countdown and trigger a refresh
      // QR_REFRESH_MARGIN_SECS before the token expires.
      interval = setInterval(tick, 1000);
    };
    const stop = () => {
      if (interval !== null) {
        clearInterval(interval);
        interval = null;
      }
    };

    const sync = () => {
      if (document.visibilityState === "visible") start();
      else stop();
    };

    sync();
    document.addEventListener("visibilitychange", sync);

    return () => {
      // Signal any in-flight pairingQrSvg() call not to setState after unmount.
      qrCancelledRef.current = true;
      stop();
      document.removeEventListener("visibilitychange", sync);
    };
  // generateQr is stable (useCallback with no deps), so this only runs once.
  }, [generateQr]);

  // --- Load peers ---
  const loadPeers = useCallback(async () => {
    setLoadState("loading");
    try {
      const { peers: fetched } = await api.listPeers();
      // Deduplicate by fingerprint (daemon-side fix may not always be deployed).
      const seen = new Set<string>();
      const deduped = fetched.filter((p) => {
        if (seen.has(p.fingerprint)) return false;
        seen.add(p.fingerprint);
        return true;
      });
      // Read from ref so we always use the latest fingerprint even if ownState
      // hasn't re-rendered yet — avoids the stale-closure bug (P1 audit finding).
      const ownFp = ownFpRef.current;
      // Don't show this device in the peers list.
      const filteredPeers = deduped.filter(
        (p) => ownFp === null || p.fingerprint !== ownFp
      );
      setPeers(filteredPeers);
      // Preserve transient per-row state across reloads; discard rows no longer
      // in the peer list so stale state doesn't accumulate.
      const liveFingerprints = new Set(filteredPeers.map((p) => p.fingerprint));
      setRowState((prev) => {
        const next: Record<string, DeviceRowState> = {};
        for (const fp of liveFingerprints) {
          if (prev[fp]) next[fp] = prev[fp];
        }
        return next;
      });

      // A-4: stamp when we last got fresh data so the 1 s clock can compute
      // live elapsed seconds since the daemon snapshot.
      peersFetchedAtRef.current = Math.floor(Date.now() / 1000);
      setLoadState("ready");
    } catch (e) {
      // A transport failure means the daemon is genuinely unreachable.
      if (e instanceof IpcError && e.code === "daemon_offline") {
        setLoadState("offline");
        return;
      }
      // The daemon answered but list_peers failed. Probe `status` to tell a
      // DB-degraded daemon (recoverable via History → Reset database) apart from
      // a generic error — previously both fell through to "offline" and the
      // degraded case was mislabeled "Daemon not running."
      const probe = await probeStatus();
      if (probe.kind === "offline") {
        setLoadState("offline");
      } else if (probe.kind === "degraded") {
        setLoadState("degraded");
      } else {
        setLoadState("error");
      }
    }
  // ownFpRef is a ref — reading it inside the callback is always current.
  // No dep on ownState avoids a stale-closure re-creation on every render.
  }, []);

  useEffect(() => {
    void loadPeers();
  }, [loadPeers]);

  // Poll peers every 10 s so the online dot refreshes without user interaction.
  // Clears the interval on unmount to avoid timer leaks (matches existing pattern).
  const PEERS_POLL_MS = 10_000;
  useEffect(() => {
    const id = setInterval(() => { void loadPeers(); }, PEERS_POLL_MS);
    return () => { clearInterval(id); };
  }, [loadPeers]);

  // --- Discover LAN peers ---
  // Poll list_discovered every DISCOVERED_POLL_MS while the view is open. Only
  // unpaired, pairable peers are shown (paired ones live in the list above).
  const loadDiscovered = useCallback(async () => {
    try {
      const { devices } = await api.listDiscovered();
      const ownFp = ownFpRef.current;
      const unpaired = devices.filter(
        (d) => !d.paired && (ownFp === null || d.device_id !== ownFp)
      );
      setDiscovered(unpaired);
    } catch (e) {
      // Discovery is best-effort (e.g. P2P disabled). Don't surface as a hard
      // error — just show an empty list. A real daemon-offline state is already
      // handled by the peers loader above.
      if (e instanceof IpcError && e.code === "daemon_offline") {
        setDiscovered([]);
      }
    }
  }, []);

  useEffect(() => {
    void loadDiscovered();
    const id = setInterval(() => { void loadDiscovered(); }, DISCOVERED_POLL_MS);
    return () => { clearInterval(id); };
  }, [loadDiscovered]);

  // HB-9: manual rescan — restart the daemon's mDNS browse in place and refresh
  // the discovered list with the fresh snapshot it returns.
  const handleRescan = useCallback(async () => {
    if (rescanning) return;
    setRescanning(true);
    setDiscoverError(null);
    try {
      const { devices } = await api.rescanDiscovered();
      const ownFp = ownFpRef.current;
      const unpaired = devices.filter(
        (d) => !d.paired && (ownFp === null || d.device_id !== ownFp)
      );
      setDiscovered(unpaired);
    } catch (e) {
      if (e instanceof IpcError && e.code === "daemon_offline") {
        setDiscovered([]);
      } else {
        setDiscoverError(e instanceof Error ? e.message : "Rescan failed");
      }
    } finally {
      setRescanning(false);
    }
  }, [rescanning]);
  // NOTE: the responder-side incoming-pairing detection used to live here as a
  // view-level pair_get_sas poll, but that only ran while the Devices tab was
  // mounted. It is now handled by an always-on poll in the Tauri backend
  // (`spawn_incoming_pairing_poller`) which emits an "incoming-pairing" event;
  // App.tsx routes the payload here via the `incomingPairing` prop → seeds
  // `responderPairing` (above) → opens the modal regardless of the active tab.

  // Begin a discovery-initiated SAS pairing, then open the SAS modal.
  const handlePairDiscovered = useCallback(async (device: DiscoveredDevice) => {
    if (pairStarting || pairingDevice !== null) return;
    setDiscoverError(null);
    setPairStarting(true);
    try {
      await api.pairWithDiscovered(device.device_id);
      setPairingDevice(device);
    } catch (e) {
      if (e instanceof IpcError && e.code === "rate_limited") {
        setDiscoverError("Another pairing is already in progress.");
      } else {
        const msg = ipcErrorMessage(e, "Failed to start pairing");
        setDiscoverError(msg);
      }
    } finally {
      setPairStarting(false);
    }
  }, [pairStarting, pairingDevice]);

  // Close the SAS modal and refresh both lists (a freshly paired device should
  // move from "Discovered" to the paired list). Clears both the initiator
  // (pairingDevice) and responder (incomingPairing) modal state so the
  // background poll can detect a subsequent new pairing session.
  const handleClosePairing = useCallback(() => {
    setPairingDevice(null);
    setResponderPairing(null);
    void loadPeers();
    void loadDiscovered();
  }, [loadPeers, loadDiscovered]);

  // Unmount guard for handleUnpair / handleRevoke — prevents setState after
  // the component unmounts if the user navigates away mid-request (P2 finding).
  const peerActionCancelledRef = useRef(false);
  useEffect(() => {
    peerActionCancelledRef.current = false;
    return () => { peerActionCancelledRef.current = true; };
  }, []);

  // Clear handler-scheduled message timer on unmount so a late tick never
  // calls setState on an unmounted component (UI memory leak).
  useEffect(() => {
    return () => {
      if (globalMsgTimer.current !== null) clearTimeout(globalMsgTimer.current);
    };
  }, []);

  // --- Row helpers ---
  const setRowPending = (fingerprint: string, pending: boolean) => {
    setRowState((prev) => ({
      ...prev,
      [fingerprint]: {
        revokedAt: prev[fingerprint]?.revokedAt ?? null,
        error: prev[fingerprint]?.error ?? null,
        pending,
      },
    }));
  };

  const setRowError = (fingerprint: string, error: string) => {
    setRowState((prev) => ({
      ...prev,
      [fingerprint]: {
        revokedAt: prev[fingerprint]?.revokedAt ?? null,
        pending: false,
        error,
      },
    }));
  };

  const handleUnpair = async (fingerprint: string) => {
    setRowPending(fingerprint, true);
    try {
      await api.unpairPeer(fingerprint);
      if (peerActionCancelledRef.current) return;
      await loadPeers();
    } catch (err) {
      if (peerActionCancelledRef.current) return;
      const msg = ipcErrorMessage(err, "Unpair failed");
      setRowError(fingerprint, msg);
    }
  };

  // Plain revoke: P2P-only (mTLS allowlist + denylist). Does NOT cut off
  // cloud/relay — that requires a sync-key rotation (see handleRevokeAndRotate).
  const handleRevoke = async (fingerprint: string) => {
    setRevokePrompt(null);
    setRowPending(fingerprint, true);
    try {
      const { revoked_at } = await api.revokePeer(fingerprint);
      if (peerActionCancelledRef.current) return;
      setRowState((prev) => ({
        ...prev,
        [fingerprint]: { revokedAt: revoked_at, pending: false, error: null },
      }));
      await loadPeers();
    } catch (err) {
      if (peerActionCancelledRef.current) return;
      const msg = ipcErrorMessage(err, "Revoke failed");
      setRowError(fingerprint, msg);
    }
  };

  // C-P0-4: revoke from P2P AND rotate the sync key so the revoked device is
  // also cut off from cloud/relay sync. `revoke_and_rotate` derives the new key
  // first, so a too-short passphrase fails before any revocation is applied.
  const handleRevokeAndRotate = async (fingerprint: string) => {
    if (revokeBusy) return;
    setRevokeBusy(true);
    setRowPending(fingerprint, true);
    try {
      const { revoked_at } = await api.revokeAndRotate(fingerprint, rotatePassphrase);
      if (peerActionCancelledRef.current) return;
      setRevokePrompt(null);
      setRotatePassphrase("");
      setRowState((prev) => ({
        ...prev,
        [fingerprint]: { revokedAt: revoked_at, pending: false, error: null },
      }));
      if (globalMsgTimer.current !== null) clearTimeout(globalMsgTimer.current);
      setGlobalMsg({
        text: "Revoked & rotated sync key — re-provision remaining devices",
        isError: false,
      });
      globalMsgTimer.current = setTimeout(() => setGlobalMsg(null), 5000);
      await loadPeers();
    } catch (err) {
      if (peerActionCancelledRef.current) return;
      const msg = ipcErrorMessage(err, "Revoke & rotate failed");
      setRowError(fingerprint, msg);
    } finally {
      if (!peerActionCancelledRef.current) setRevokeBusy(false);
    }
  };

  const handleRevokeAllConfirmed = async () => {
    setRevokeAllConfirm(false);
    setRevokeAllPending(true);
    try {
      const result = await api.revokeAllPeers();
      const n = result.revoked ?? 0;
      if (globalMsgTimer.current !== null) clearTimeout(globalMsgTimer.current);
      setGlobalMsg({ text: `Revoked ${n} device${n === 1 ? "" : "s"}`, isError: false });
      globalMsgTimer.current = setTimeout(() => setGlobalMsg(null), 3000);
      await loadPeers();
    } catch (err) {
      const msg = ipcErrorMessage(err, "Revoke all failed");
      if (globalMsgTimer.current !== null) clearTimeout(globalMsgTimer.current);
      setGlobalMsg({ text: msg, isError: true });
      globalMsgTimer.current = setTimeout(() => setGlobalMsg(null), 4000);
    } finally {
      setRevokeAllPending(false);
    }
  };

  // --- Actions bar ---
  const actions = (
    <div className="flex items-center gap-2">
      {globalMsg !== null && (
        <span
          className={[
            "text-[12px]",
            globalMsg.isError ? "text-ide-danger" : "text-ide-success",
          ].join(" ")}
        >
          {globalMsg.text}
        </span>
      )}
      {revokeAllConfirm ? (
        <span className="flex items-center gap-1.5 text-[12px]">
          <span className="text-ide-dim">Revoke all?</span>
          {/* puf4: solid-danger for primary destructive confirm (Revoke all) */}
          <button
            onClick={() => void handleRevokeAllConfirmed()}
            className="rounded-ide bg-ide-danger px-2.5 py-1 text-[12px] font-medium text-white hover:bg-ide-danger/85 shadow-ide-xs"
          >
            Yes
          </button>
          <button
            onClick={() => setRevokeAllConfirm(false)}
            className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-dim hover:bg-ide-raised hover:text-ide-text shadow-ide-xs"
          >
            No
          </button>
        </span>
      ) : (
        <button
          onClick={() => setRevokeAllConfirm(true)}
          disabled={revokeAllPending || loadState !== "ready" || peers.length === 0}
          className="rounded-ide border border-ide-danger/35 bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-danger hover:bg-ide-raised hover:border-ide-danger/60 shadow-ide-xs disabled:cursor-not-allowed disabled:opacity-40"
        >
          {revokeAllPending ? "Revoking..." : "Revoke all"}
        </button>
      )}
    </div>
  );

  // --- Offline state ---
  if (loadState === "offline") {
    return (
      <ViewShell title="Devices" actions={actions}>
        <EmptyState
          className="h-full"
          icon={<Zap width={28} height={28} strokeWidth={1.5} />}
          title="Clipboard service offline"
          body="The daemon is not running."
          action={<div className="mt-1"><RestartDaemonButton onRestarted={() => void loadPeers()} /></div>}
        />
      </ViewShell>
    );
  }

  // --- Degraded state (daemon up, DB unavailable) ---
  if (loadState === "degraded") {
    return (
      <ViewShell title="Devices" actions={actions}>
        <EmptyState
          className="h-full"
          icon={<AlertCircle width={28} height={28} strokeWidth={1.5} className="text-ide-warning" />}
          title="Database degraded"
          body="Device list unavailable. Reset the database in History to recover."
          action={<div className="mt-1"><RestartDaemonButton onRestarted={() => void loadPeers()} /></div>}
        />
      </ViewShell>
    );
  }

  // --- Generic error state ---
  if (loadState === "error") {
    return (
      <ViewShell title="Devices" actions={actions}>
        <EmptyState
          className="h-full"
          icon={<AlertCircle width={28} height={28} strokeWidth={1.5} />}
          title="Failed to load devices"
          body="Try restarting the daemon."
          action={<div className="mt-1"><RestartDaemonButton onRestarted={() => void loadPeers()} /></div>}
        />
      </ViewShell>
    );
  }

  return (
    <ViewShell title="Devices" actions={actions}>
      {/* ── Devices section header — with online count ──────────── */}
      {/* The online count uses the live presence store when available (updated
          ~every 1 s from peer events); falls back to peer.online from the last
          10 s list_peers poll when the store has no entry yet. */}
      <div className="mb-2 flex items-center justify-between">
        <p className="text-[11px] font-medium uppercase tracking-wider text-ide-faint">
          Devices
        </p>
        {loadState === "ready" && peers.length > 0 && (
          <span className="text-[11px] text-ide-faint">
            <span className="inline-block w-2 h-2 rounded-full bg-ide-success mr-1 align-middle" />
            {peers.filter((p) => {
              const live = presenceOnline[p.fingerprint];
              return live !== undefined ? live : p.online === true;
            }).length} online
          </span>
        )}
      </div>

      {/* ── Single unified device list (this Mac first, then peers) ── */}
      {/* surface-card glass: the list container is a frosted layer over the aurora. */}
      <div className="surface-card flex flex-col divide-y divide-ide-divider rounded-ide">
        {/* This device — always first */}
        {ownState.status === "loading" && (
          <div className="px-3 py-2.5">
            <p className="text-[13px] text-ide-faint animate-pulse">Loading...</p>
          </div>
        )}
        {ownState.status === "offline" && (
          <div className="px-3 py-2.5">
            <p className="text-[13px] text-ide-danger">Daemon not running.</p>
          </div>
        )}
        {ownState.status === "ready" && (
          <ThisDeviceCard info={ownState.info} />
        )}

        {/* Paired peers */}
        {loadState === "loading" && (
          <div className="px-3 py-2.5">
            <p className="text-[13px] text-ide-faint animate-pulse">Loading peers...</p>
          </div>
        )}
        {loadState === "ready" && peers.length === 0 && (
          <div className="px-3 py-3 flex items-center gap-2">
            {/* Briefcase icon via lucide-react (§9: replace inline SVGs) */}
            <Briefcase size={14} strokeWidth={1.5} className="text-ide-faint shrink-0" />
            <p className="text-[13px] text-ide-dim">No paired devices yet.</p>
          </div>
        )}
        {loadState === "ready" &&
          peers.map((peer) => {
            // A-4: advance the daemon's last_seen_secs snapshot by the number of
            // seconds elapsed since we last fetched peers, so the "Xm ago" tooltip
            // updates every 1 s tick instead of jumping only on the 10 s poll.
            const elapsed = nowSecs - peersFetchedAtRef.current;
            const rawSecs = peer.last_seen_secs ?? -1;
            const liveLastSeenSecs = rawSecs >= 0 ? rawSecs + elapsed : undefined;
            return (
              <PeerRow
                key={peer.fingerprint}
                peer={peer}
                rowSt={rowState[peer.fingerprint]}
                liveLastSeenSecs={liveLastSeenSecs}
                // Live presence overlay: if the event-broadcast store has seen
                // a connect/disconnect for this peer, use it; otherwise fall back
                // to the value from the last `list_peers` poll.
                liveOnline={presenceOnline[peer.fingerprint]}
                onUnpair={(fp) => void handleUnpair(fp)}
                onRevoke={(fp) => {
                  setRotatePassphrase("");
                  setRevokePrompt({
                    fingerprint: fp,
                    name: peer.name || `Device ${fp.slice(0, 8)}`,
                  });
                }}
              />
            );
          })}
      </div>

      {/* ── Discovered on your network ─────────────────────────── */}
      {/* HB-9: header + Refresh button always render so a manual rescan is
          reachable even when passive polling hasn't surfaced any peer yet. */}
      <div className="mb-2 mt-5 flex items-center justify-between">
        <p className="text-[11px] font-medium uppercase tracking-wider text-ide-faint">
          Discovered on your network
        </p>
        <button
          type="button"
          onClick={() => void handleRescan()}
          disabled={rescanning}
          aria-label={rescanning ? "Scanning…" : "Rescan local network"}
          className="flex items-center gap-1 rounded-ide px-2 py-0.5 text-[11px] font-medium text-ide-accent hover:bg-ide-hover disabled:opacity-50 disabled:cursor-default"
          title="Rescan the local network for devices"
        >
          {/* RefreshCw from lucide-react; spins while rescanning; reduced-motion: static */}
          <RefreshCw
            size={11}
            strokeWidth={2.2}
            aria-hidden="true"
            className={rescanning ? "animate-spin motion-reduce:animate-none" : ""}
          />
          {rescanning ? "Scanning…" : "Refresh"}
        </button>
      </div>
      {discovered.length > 0 ? (
        <div className="surface-card flex flex-col divide-y divide-ide-divider rounded-ide">
          {discovered.map((device) => (
            <DiscoveredRow
              key={device.device_id}
              device={device}
              onPair={(d) => void handlePairDiscovered(d)}
              busy={pairStarting || pairingDevice !== null}
            />
          ))}
        </div>
      ) : (
        <p className="text-[11px] text-ide-faint">No devices found on the network yet.</p>
      )}
      {discoverError !== null && (
        <p className="mt-2 text-[11px] text-ide-danger">{discoverError}</p>
      )}

      {/* ── Divider ────────────────────────────────────────────── */}
      <div className="my-5 border-t border-ide-divider" />

      {/* ── Pair via QR — full width, compact code ───────────────── */}
      <p className="mb-2 text-[11px] font-medium uppercase tracking-wider text-ide-faint">
        Pair a new device
      </p>

      <section className="surface-card rounded-ide-lg p-4 space-y-3 shadow-ide-sm">
        {qrState.status === "loading" && (
          <p className="text-[12px] text-ide-dim animate-pulse">Generating...</p>
        )}

        {qrState.status === "ready" && (
          <div className="flex items-start gap-5">
            {/* QR code — SVG comes from our own Tauri backend and never
                contains remote markup — dangerouslySetInnerHTML is safe here.
                Privacy-first: blurred by default, revealed on click (spec §10). */}
            <div
              className="relative shrink-0 rounded-ide bg-white p-2 overflow-hidden"
              style={{ width: 190, height: 190 }}
            >
              <div
                className={[
                  "[&>svg]:block [&>svg]:h-full [&>svg]:w-full transition-[filter] duration-200",
                  qrBlur === "blurred" ? "blur-md" : "",
                ].join(" ")}
                style={{ width: "100%", height: "100%" }}
                // eslint-disable-next-line react/no-danger
                dangerouslySetInnerHTML={{ __html: qrState.qr.svg }}
              />
              {/* Blur overlay — click-to-reveal affordance */}
              {qrBlur === "blurred" && (
                <button
                  type="button"
                  onClick={handleQrReveal}
                  className="absolute inset-0 flex flex-col items-center justify-center gap-1.5 bg-white/10 focus:outline-none focus-visible:ring-2 focus-visible:ring-ide-accent rounded-ide"
                  aria-label="Click to reveal QR code"
                >
                  <span className="text-[11px] font-medium text-ide-dim select-none">
                    Click to reveal
                  </span>
                </button>
              )}
            </div>
            <div className="min-w-0 flex-1 space-y-2">
              {/* Payload only visible when QR is revealed */}
              {qrBlur === "revealed" && (
                <p className="select-all break-all font-mono text-[10px] text-ide-faint">
                  {qrState.qr.payload}
                </p>
              )}
              {qrSecsLeft !== null && qrSecsLeft > 0 && (
                <>
                  {/* Determinate drain bar: width drains from 100% to 0 as time runs out */}
                  {(() => {
                    const ttl = qrState.status === "ready" && qrState.qr.expires_in_secs > 0
                      ? qrState.qr.expires_in_secs
                      : QR_TTL_SECS;
                    const pct = Math.min(100, Math.max(0, (qrSecsLeft / ttl) * 100));
                    return (
                      <div className="w-full h-0.5 rounded-full bg-ide-elevated overflow-hidden">
                        <div
                          data-testid="qr-drain-bar"
                          className={[
                            "h-full rounded-full transition-[width] duration-1000 ease-linear",
                            qrSecsLeft <= 20 ? "bg-ide-warning" : "bg-ide-accent",
                          ].join(" ")}
                          style={{ width: `${pct}%` }}
                        />
                      </div>
                    );
                  })()}
                  <p className="text-[11px] text-ide-dim">
                    Expires in{" "}
                    <span className={qrSecsLeft <= 20 ? "text-ide-warning font-medium tabular-nums" : "tabular-nums"}>
                      {qrSecsLeft}s
                    </span>
                  </p>
                </>
              )}
              <p className="text-[11px] text-ide-faint">
                Scan from CopyPaste on another device to pair automatically.
              </p>
              {/* Explicit regenerate button — separate from reveal so blur state
                  is not accidentally cleared by a refresh (spec §10). */}
              <button
                type="button"
                onClick={handleQrRegenerate}
                className="flex items-center gap-1 text-[11px] text-ide-accent hover:text-ide-accent-hover focus:outline-none focus-visible:ring-1 focus-visible:ring-ide-accent rounded"
                aria-label="Regenerate pairing QR code"
              >
                <RefreshCw size={10} strokeWidth={2} aria-hidden="true" />
                Regenerate
              </button>
            </div>
          </div>
        )}

        {qrState.status === "error" && (
          <p className="text-[12px] text-ide-danger">{qrState.message}</p>
        )}

        {qrState.status === "idle" && (
          <p className="text-[12px] text-ide-dim animate-pulse">Generating pairing code...</p>
        )}
      </section>

      {/* ── SAS pairing modal (discovery-initiated, initiator). The responder
          path is handled by the dedicated `responderPairing` modal below,
          driven by the always-on backend poll → "incoming-pairing" event. */}
      {pairingDevice !== null && (
        <SasPairingModal
          device={pairingDevice ?? undefined}
          initialStatus={incomingPairing ?? undefined}
          onClose={handleClosePairing}
          onPaired={loadPeers}
        />
      )}

      {/* ── SAS pairing modal (incoming/responder) ───────────────────────
           Opened when the Tauri backend detects an inbound pairing request
           (pair_get_sas state="awaiting_sas" + role="responder") and emits
           the "incoming-pairing" event.  App.tsx switches to the Devices tab
           and passes the payload via the `incomingPairing` prop, which seeds
           `responderPairing` on mount.  Only shown when no initiator modal is
           already open (pairingDevice takes precedence). */}
      {pairingDevice === null && responderPairing !== null && (
        <SasPairingModal
          displayName={
            responderPairing.peer_device_name
              ?? responderPairing.peer_name
              ?? "A device"
          }
          initialStatus={responderPairing}
          onClose={() => {
            setResponderPairing(null);
            void loadPeers();
            void loadDiscovered();
          }}
          onPaired={loadPeers}
        />
      )}

      {/* ── C-P0-4: Revoke confirm — offer P2P-only vs cloud/relay rotation ── */}
      {revokePrompt !== null && (
        <RevokeConfirmDialog
          name={revokePrompt.name}
          fingerprint={revokePrompt.fingerprint}
          rotatePassphrase={rotatePassphrase}
          revokeBusy={revokeBusy}
          onPassphraseChange={setRotatePassphrase}
          onCancel={() => {
            setRevokePrompt(null);
            setRotatePassphrase("");
          }}
          onRevoke={(fp) => void handleRevoke(fp)}
          onRevokeAndRotate={(fp) => void handleRevokeAndRotate(fp)}
        />
      )}
    </ViewShell>
  );
}
