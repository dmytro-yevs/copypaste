import { useState, useEffect, useCallback, useRef } from "react";
import {
  api,
  IpcError,
  formatWallTime,
  formatEpochSecs,
  pairingQrSvg,
  probeStatus,
  type OwnDeviceInfo,
  type PairedDevice,
  type PairingQr,
} from "../lib/ipc";
import { ViewShell } from "../components/ViewShell";
import { RestartDaemonButton } from "../components/RestartDaemonButton";

type QrState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; qr: PairingQr; generatedAt: number }
  | { status: "error"; message: string };

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
// MetaRow — one labelled line in the rich-info block, hidden when absent
// ---------------------------------------------------------------------------

function MetaRow({ label, value }: { label: string; value: string | null | undefined }) {
  if (!value) return null;
  return (
    <p className="text-[11px] text-ide-faint">
      <span className="text-ide-dim">{label}</span>{" "}
      <span>{value}</span>
    </p>
  );
}

// ---------------------------------------------------------------------------
// ThisDeviceCard — rich identity block for the local device
// ---------------------------------------------------------------------------

function ThisDeviceCard({
  info,
  copied,
  onCopy,
}: {
  info: OwnDeviceInfo;
  copied: boolean;
  onCopy: () => void;
}) {
  return (
    <div className="px-3 py-2.5">
      {/* Name + "This Mac" badge */}
      <div className="flex flex-wrap items-center gap-1.5 mb-1">
        <p className="truncate text-[13px] font-medium text-ide-text">
          {info.device_name ?? "This Device"}
        </p>
        <span className="shrink-0 rounded px-1 py-0.5 text-[10px] font-medium bg-ide-accent/15 text-ide-accent">
          This Mac
        </span>
      </div>

      {/* Rich metadata rows — each omitted when absent */}
      <div className="mt-1 space-y-0.5">
        <MetaRow label="Model" value={info.device_model} />
        <MetaRow label="OS" value={info.os_version} />
        <MetaRow label="Version" value={info.app_version} />
        <MetaRow label="Local IP" value={info.local_ip} />

        {/* Fingerprint — full value with copy button */}
        {info.fingerprint !== null && (
          <div className="flex items-center gap-1.5 pt-0.5">
            <span
              className="select-all break-all font-mono text-[11px] text-ide-dim"
              title={info.fingerprint}
            >
              {info.fingerprint}
            </span>
            <button
              type="button"
              onClick={onCopy}
              className="shrink-0 rounded-ide border border-ide-border bg-ide-elevated px-2 py-0.5 text-[11px] text-ide-dim hover:bg-ide-hover hover:text-ide-text transition-colors"
            >
              {copied ? "Copied" : "Copy"}
            </button>
          </div>
        )}
        {info.fingerprint === null && (
          <p className="text-[11px] text-ide-faint">
            <span className="text-ide-dim">Fingerprint</span>{" "}
            <span className="text-ide-warning">P2P disabled — enable COPYPASTE_P2P=1</span>
          </p>
        )}
      </div>
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
}

function PeerRow({ peer, rowSt, onUnpair, onRevoke }: PeerRowProps) {
  const isPending = rowSt?.pending ?? false;
  const revokedAt = rowSt?.revokedAt ?? null;
  const rowError = rowSt?.error ?? null;

  // Extract IP from optional "host:port" address field.
  const ip = extractIp(peer.address);

  return (
    <div className="px-3 py-2.5 hover:bg-ide-hover">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          {/* Name */}
          <p className="truncate text-[13px] font-medium text-ide-text">
            {peer.name || `Device ${peer.fingerprint.slice(0, 8)}`}
          </p>

          <div className="mt-1 space-y-0.5">
            {/* Truncated fingerprint */}
            <p
              className="font-mono text-[11px] text-ide-dim"
              title={peer.fingerprint}
            >
              {peer.fingerprint.length > 32
                ? `${peer.fingerprint.slice(0, 16)}…${peer.fingerprint.slice(-8)}`
                : peer.fingerprint}
            </p>

            {/* IP address (from P2P address field) */}
            {ip !== null && (
              <p className="text-[11px] text-ide-faint">
                <span className="text-ide-dim">IP</span>{" "}
                <span className="font-mono">{ip}</span>
              </p>
            )}

            {/* Paired date */}
            {(peer.added_at ?? 0) > 0 && (
              <p className="text-[11px] text-ide-faint">
                Paired {formatEpochSecs(peer.added_at)}
              </p>
            )}

            {/* Revoked / error states */}
            {revokedAt !== null && (
              <p className="text-[11px] text-ide-accent">
                Revoked · {formatWallTime(revokedAt)}
              </p>
            )}
            {rowError !== null && (
              <p className="text-[11px] text-ide-danger">{rowError}</p>
            )}
          </div>
        </div>

        {/* Actions */}
        <div className="flex shrink-0 items-center gap-1.5 pt-0.5">
          <button
            onClick={() => onUnpair(peer.fingerprint)}
            disabled={isPending}
            className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-text hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
          >
            {isPending ? "…" : "Unpair"}
          </button>
          <button
            onClick={() => onRevoke(peer.fingerprint)}
            disabled={isPending}
            className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-danger hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
          >
            {isPending ? "…" : "Revoke"}
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
// Main view
// ---------------------------------------------------------------------------

// Pairing token TTL from the daemon (PAKE_SESSION_TTL = 120 s).
// We refresh 15 s before expiry to ensure a valid code is always on-screen.
const QR_TTL_SECS = 120;
const QR_REFRESH_MARGIN_SECS = 15;

// ---------------------------------------------------------------------------
// Main view
// ---------------------------------------------------------------------------

export function DevicesView() {
  // --- Own device info ---
  const [ownState, setOwnState] = useState<OwnDeviceState>({ status: "loading" });
  const [copied, setCopied] = useState(false);

  // --- Paired peers ---
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [peers, setPeers] = useState<PairedDevice[]>([]);
  const [rowState, setRowState] = useState<Record<string, DeviceRowState>>({});
  const [revokeAllPending, setRevokeAllPending] = useState(false);
  const [revokeAllConfirm, setRevokeAllConfirm] = useState(false);
  const [globalMsg, setGlobalMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const globalMsgTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // --- QR pairing ---
  const [qrState, setQrState] = useState<QrState>({ status: "idle" });
  // Countdown seconds remaining until the current QR expires (display only).
  const [qrSecsLeft, setQrSecsLeft] = useState<number | null>(null);
  // Ref so the auto-refresh timer can read the latest qrState without a
  // stale-closure problem — we write it in parallel with the React state.
  const qrStateRef = useRef<QrState>({ status: "idle" });

  const generateQr = useCallback(async () => {
    setQrState({ status: "loading" });
    qrStateRef.current = { status: "loading" };
    setQrSecsLeft(null);
    try {
      const qr = await pairingQrSvg();
      const next: QrState = { status: "ready", qr, generatedAt: Date.now() };
      setQrState(next);
      qrStateRef.current = next;
      setQrSecsLeft(qr.expires_in_secs > 0 ? qr.expires_in_secs : QR_TTL_SECS);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to generate pairing code";
      const next: QrState = { status: "error", message };
      setQrState(next);
      qrStateRef.current = next;
    }
  }, []);

  // --- Load own device info ---
  useEffect(() => {
    let cancelled = false;
    setOwnState({ status: "loading" });
    api.getOwnDeviceInfo().then(
      (info) => {
        if (!cancelled) setOwnState({ status: "ready", info });
      },
      (err: unknown) => {
        if (cancelled) return;
        const code = err instanceof IpcError ? err.code : null;
        if (code === "daemon_offline") {
          setOwnState({ status: "offline" });
        } else {
          // Daemon is up but method may not exist on older daemon builds —
          // treat as offline so the UI still shows the fingerprint section.
          setOwnState({ status: "offline" });
        }
      }
    );
    return () => { cancelled = true; };
  }, []);

  // Auto-generate QR on mount, auto-refresh before expiry.
  useEffect(() => {
    void generateQr();

    // Tick every second: update the countdown and trigger a refresh
    // QR_REFRESH_MARGIN_SECS before the token expires.
    const interval = setInterval(() => {
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
    }, 1000);

    return () => clearInterval(interval);
  // generateQr is stable (useCallback with no deps), so this only runs once.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
      const ownFp =
        ownState.status === "ready" ? ownState.info.fingerprint : null;
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
  }, [ownState]);

  useEffect(() => {
    void loadPeers();
  }, [loadPeers]);

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
      await loadPeers();
    } catch (err) {
      const msg = err instanceof IpcError ? err.message : "Unpair failed";
      setRowError(fingerprint, msg);
    }
  };

  const handleRevoke = async (fingerprint: string) => {
    setRowPending(fingerprint, true);
    try {
      const { revoked_at } = await api.revokePeer(fingerprint);
      setRowState((prev) => ({
        ...prev,
        [fingerprint]: { revokedAt: revoked_at, pending: false, error: null },
      }));
      await loadPeers();
    } catch (err) {
      const msg = err instanceof IpcError ? err.message : "Revoke failed";
      setRowError(fingerprint, msg);
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
      const msg = err instanceof IpcError ? err.message : "Revoke all failed";
      if (globalMsgTimer.current !== null) clearTimeout(globalMsgTimer.current);
      setGlobalMsg({ text: msg, isError: true });
      globalMsgTimer.current = setTimeout(() => setGlobalMsg(null), 4000);
    } finally {
      setRevokeAllPending(false);
    }
  };

  // --- Fingerprint copy ---
  function handleCopy() {
    if (ownState.status !== "ready" || ownState.info.fingerprint === null) return;
    const fp = ownState.info.fingerprint;
    navigator.clipboard.writeText(fp).then(
      () => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      },
      () => {
        // Fallback for restricted clipboard contexts.
        const el = document.createElement("textarea");
        el.value = fp;
        el.style.position = "fixed";
        el.style.opacity = "0";
        document.body.appendChild(el);
        el.select();
        document.execCommand("copy");
        document.body.removeChild(el);
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      }
    );
  }

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
          <button
            onClick={() => void handleRevokeAllConfirmed()}
            className="rounded-ide border border-ide-danger/40 bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-danger hover:bg-ide-raised hover:border-ide-danger/60 shadow-ide-xs"
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
          {revokeAllPending ? "Revoking…" : "Revoke all"}
        </button>
      )}
    </div>
  );

  // --- Offline state ---
  if (loadState === "offline") {
    return (
      <ViewShell title="Devices" actions={actions}>
        <div className="flex h-full flex-col items-center justify-center gap-2 px-6 text-center">
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-ide-faint">
            <path d="M13 10V3L4 14h7v7l9-11h-7z" />
          </svg>
          <p className="text-[13px] text-ide-dim">Clipboard service offline</p>
          <p className="text-[11px] text-ide-faint">The daemon is not running.</p>
          <div className="mt-1">
            <RestartDaemonButton onRestarted={() => void loadPeers()} />
          </div>
        </div>
      </ViewShell>
    );
  }

  // --- Degraded state (daemon up, DB unavailable) ---
  if (loadState === "degraded") {
    return (
      <ViewShell title="Devices" actions={actions}>
        <div className="flex h-full flex-col items-center justify-center gap-2 px-6 text-center">
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-ide-warning">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
          <p className="text-[13px] text-ide-dim">Database degraded</p>
          <p className="text-[11px] text-ide-faint">Device list unavailable. Reset the database in History to recover.</p>
          <div className="mt-1">
            <RestartDaemonButton onRestarted={() => void loadPeers()} />
          </div>
        </div>
      </ViewShell>
    );
  }

  // --- Generic error state ---
  if (loadState === "error") {
    return (
      <ViewShell title="Devices" actions={actions}>
        <div className="flex h-full flex-col items-center justify-center gap-2 px-6 text-center">
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-ide-faint">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
          <p className="text-[13px] text-ide-dim">Failed to load devices</p>
          <p className="text-[11px] text-ide-faint">Try restarting the daemon.</p>
          <div className="mt-1">
            <RestartDaemonButton onRestarted={() => void loadPeers()} />
          </div>
        </div>
      </ViewShell>
    );
  }

  // --- Already-paired indicator for QR panel ---
  // If the user shows a QR code but they already have paired devices, surface a
  // note. The daemon will reject a duplicate-fingerprint scan with an error, but
  // surfacing this proactively avoids confusion on the displaying side.
  const alreadyPairedCount = peers.length;

  return (
    <ViewShell title="Devices" actions={actions}>
      {/* ── Devices section header ──────────────────────────────── */}
      <p className="mb-2 text-[11px] font-medium uppercase tracking-wider text-ide-faint">
        Devices
      </p>

      {/* ── Single unified device list (this Mac first, then peers) ── */}
      <div className="flex flex-col divide-y divide-ide-divider rounded-ide border border-ide-border bg-ide-panel/60">
        {/* This device — always first */}
        {ownState.status === "loading" && (
          <div className="px-3 py-2.5">
            <p className="text-[13px] text-ide-faint animate-pulse">Loading…</p>
          </div>
        )}
        {ownState.status === "offline" && (
          <div className="px-3 py-2.5">
            <p className="text-[13px] text-ide-danger">Daemon not running.</p>
          </div>
        )}
        {ownState.status === "ready" && (
          <ThisDeviceCard
            info={ownState.info}
            copied={copied}
            onCopy={handleCopy}
          />
        )}

        {/* Paired peers */}
        {loadState === "loading" && (
          <div className="px-3 py-2.5">
            <p className="text-[13px] text-ide-faint animate-pulse">Loading peers…</p>
          </div>
        )}
        {loadState === "ready" && peers.length === 0 && (
          <div className="px-3 py-3 flex items-center gap-2">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-ide-faint shrink-0">
              <rect x="2" y="7" width="20" height="14" rx="2" />
              <path d="M16 7V5a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v2" />
            </svg>
            <p className="text-[13px] text-ide-dim">No paired devices yet.</p>
          </div>
        )}
        {loadState === "ready" &&
          peers.map((peer) => (
            <PeerRow
              key={peer.fingerprint}
              peer={peer}
              rowSt={rowState[peer.fingerprint]}
              onUnpair={(fp) => void handleUnpair(fp)}
              onRevoke={(fp) => void handleRevoke(fp)}
            />
          ))}
      </div>

      {/* ── Divider ────────────────────────────────────────────── */}
      <div className="my-5 border-t border-ide-divider" />

      {/* ── Pair via QR — full width, compact code ───────────────── */}
      <p className="mb-2 text-[11px] font-medium uppercase tracking-wider text-ide-faint">
        Pair a new device
      </p>

      <section className="rounded-ide-lg border border-ide-border bg-ide-elevated p-4 space-y-3 shadow-ide-sm">
        {/* Already-paired warning: shown when QR is visible and peers exist.
            A device scanning this QR that is already paired will be rejected by
            the daemon ("peer already paired") — surface the context proactively. */}
        {qrState.status === "ready" && alreadyPairedCount > 0 && (
          <div className="flex items-start gap-2 rounded-ide border border-ide-warning/40 bg-ide-warning/5 px-3 py-2">
            <span className="text-[11px] text-ide-warning">
              {alreadyPairedCount === 1
                ? "1 device is already paired. Scanning this code from an already-paired device will have no effect."
                : `${alreadyPairedCount} devices are already paired. Scanning from an already-paired device will have no effect.`}
            </span>
          </div>
        )}

        {qrState.status === "idle" && (
          <p className="text-[12px] text-ide-dim">
            Generate a single-use code, then scan it from the CopyPaste app on
            another device to pair automatically — no typing a password.
          </p>
        )}

        {qrState.status === "loading" && (
          <p className="text-[12px] text-ide-dim animate-pulse">Generating…</p>
        )}

        {qrState.status === "ready" && (
          <div className="flex items-start gap-5">
            {/* QR code — constrained to ~190 px so it stays scannable but doesn't
                dominate the view. The SVG comes from our own Tauri backend (qrcode
                crate) and never contains remote markup — dangerouslySetInnerHTML
                is safe here. */}
            <div
              className="shrink-0 rounded-ide bg-white p-2 overflow-hidden [&>svg]:block [&>svg]:h-full [&>svg]:w-full"
              style={{ width: 190, height: 190 }}
              // eslint-disable-next-line react/no-danger
              dangerouslySetInnerHTML={{ __html: qrState.qr.svg }}
            />
            <div className="min-w-0 flex-1 space-y-2">
              <p className="select-all break-all font-mono text-[10px] text-ide-faint">
                {qrState.qr.payload}
              </p>
              {qrSecsLeft !== null && qrSecsLeft > 0 && (
                <p className="text-[11px] text-ide-dim">
                  Expires in{" "}
                  <span
                    // Highlight countdown in warning colour when under 20 s.
                    className={qrSecsLeft <= 20 ? "text-ide-warning font-medium" : ""}
                  >
                    {qrSecsLeft}s
                  </span>
                  {" "}· auto-refreshes before expiry
                </p>
              )}
            </div>
          </div>
        )}

        {qrState.status === "error" && (
          <p className="text-[12px] text-ide-danger">{qrState.message}</p>
        )}

        <button
          type="button"
          onClick={() => void generateQr()}
          disabled={qrState.status === "loading"}
          className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-text hover:bg-ide-raised hover:text-ide-text shadow-ide-xs disabled:cursor-not-allowed disabled:opacity-40"
        >
          {qrState.status === "loading"
            ? "Generating…"
            : qrState.status === "ready"
              ? "Regenerate code"
              : "Show pairing code"}
        </button>
      </section>
    </ViewShell>
  );
}
