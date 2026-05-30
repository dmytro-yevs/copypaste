import { useState, useEffect, useCallback, useRef } from "react";
import {
  api,
  IpcError,
  formatWallTime,
  formatEpochSecs,
  pairingQrSvg,
  probeStatus,
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

interface DeviceRowState {
  revokedAt: number | null;
  pending: boolean;
  error: string | null;
}

type FingerprintState =
  | { status: "loading" }
  | { status: "ready"; fingerprint: string }
  | { status: "degraded"; reason: string | null }
  | { status: "offline" };

// Pairing token TTL from the daemon (PAKE_SESSION_TTL = 120 s).
// We refresh 15 s before expiry to ensure a valid code is always on-screen.
const QR_TTL_SECS = 120;
const QR_REFRESH_MARGIN_SECS = 15;

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
// DeviceRow — renders one device entry (this device or a peer)
// ---------------------------------------------------------------------------

interface DeviceRowProps {
  peer: PairedDevice;
  isThisDevice: boolean;
  rowSt: DeviceRowState | undefined;
  onUnpair: (fp: string) => void;
  onRevoke: (fp: string) => void;
  ownFingerprint: string | null;
  fpCopied: boolean;
  onCopyFingerprint: () => void;
}

function DeviceRow({
  peer,
  isThisDevice,
  rowSt,
  onUnpair,
  onRevoke,
  ownFingerprint: _ownFingerprint,
  fpCopied,
  onCopyFingerprint,
}: DeviceRowProps) {
  const isPending = rowSt?.pending ?? false;
  const revokedAt = rowSt?.revokedAt ?? null;
  const rowError = rowSt?.error ?? null;
  const ip = extractIp(peer.address);

  return (
    <div className="px-3 py-2.5 hover:bg-ide-hover">
      {/* Name row */}
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5">
            <p className="truncate text-[13px] font-medium text-ide-text">
              {peer.name || `Device ${peer.fingerprint.slice(0, 8)}`}
            </p>
            {isThisDevice && (
              <span className="shrink-0 rounded px-1 py-0.5 text-[10px] font-medium bg-ide-accent/15 text-ide-accent">
                This Mac
              </span>
            )}
          </div>

          {/* Rich metadata block */}
          <div className="mt-1 space-y-0.5">
            {/* Fingerprint — full value for this device (copy button), truncated for peers */}
            {isThisDevice ? (
              <div className="flex items-center gap-1.5">
                <span
                  className="select-all break-all font-mono text-[11px] text-ide-dim"
                  title={peer.fingerprint}
                >
                  {peer.fingerprint}
                </span>
                <button
                  type="button"
                  onClick={onCopyFingerprint}
                  className="shrink-0 rounded-ide border border-ide-border bg-ide-elevated px-2 py-0.5 text-[11px] text-ide-dim hover:bg-ide-hover hover:text-ide-text transition-colors"
                >
                  {fpCopied ? "Copied" : "Copy"}
                </button>
              </div>
            ) : (
              <p
                className="font-mono text-[11px] text-ide-dim"
                title={peer.fingerprint}
              >
                {peer.fingerprint.length > 32
                  ? `${peer.fingerprint.slice(0, 16)}…${peer.fingerprint.slice(-8)}`
                  : peer.fingerprint}
              </p>
            )}

            {/* IP address */}
            {ip !== null && (
              <p className="text-[11px] text-ide-faint">
                <span className="text-ide-dim">IP</span>{" "}
                <span className="font-mono">{ip}</span>
              </p>
            )}

            {/* Paired date */}
            {peer.added_at > 0 && (
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

        {/* Actions — only for peer devices, not this device */}
        {!isThisDevice && (
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
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main view
// ---------------------------------------------------------------------------

export function DevicesView() {
  // --- Devices state ---
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [peers, setPeers] = useState<PairedDevice[]>([]);
  const [rowState, setRowState] = useState<Record<string, DeviceRowState>>({});
  const [revokeAllPending, setRevokeAllPending] = useState(false);
  const [revokeAllConfirm, setRevokeAllConfirm] = useState(false);
  const [globalMsg, setGlobalMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const globalMsgTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // --- Own fingerprint ---
  const [fpState, setFpState] = useState<FingerprintState>({ status: "loading" });
  const [copied, setCopied] = useState(false);

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

  // Auto-generate on mount, auto-refresh before expiry.
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
      setPeers(deduped);
      // Preserve transient per-row state across reloads; discard rows no longer
      // in the peer list so stale state doesn't accumulate.
      const liveFingerprints = new Set(deduped.map((p) => p.fingerprint));
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
  }, []);

  // --- Load own fingerprint ---
  useEffect(() => {
    let cancelled = false;
    setFpState({ status: "loading" });
    api.getOwnFingerprint().then(
      ({ fingerprint }) => {
        if (!cancelled) setFpState({ status: "ready", fingerprint });
      },
      (err: unknown) => {
        if (cancelled) return;
        const code = err instanceof IpcError ? err.code : null;
        if (code === "daemon_offline") {
          setFpState({ status: "offline" });
          return;
        }
        // Daemon answered but the call failed — distinguish a DB-degraded daemon
        // from a true offline daemon instead of collapsing both to "offline".
        void probeStatus().then((probe) => {
          if (cancelled) return;
          if (probe.kind === "degraded")
            setFpState({ status: "degraded", reason: probe.reason });
          else setFpState({ status: "offline" });
        });
      }
    );
    return () => {
      cancelled = true;
    };
  }, []);

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

  // --- Fingerprint copy (used both standalone and inside the device row) ---
  function handleCopy() {
    if (fpState.status !== "ready") return;
    navigator.clipboard.writeText(fpState.fingerprint).then(
      () => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      },
      () => {
        // Fallback for restricted clipboard contexts.
        const el = document.createElement("textarea");
        el.value = (fpState as { status: "ready"; fingerprint: string }).fingerprint;
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

  const ownFingerprint =
    fpState.status === "ready" ? fpState.fingerprint : null;

  // --- Build the synthetic "this device" row from fingerprint state ---
  // Shown first in the list regardless of whether peers are loaded yet.
  const thisDevicePeer: PairedDevice | null =
    fpState.status === "ready"
      ? {
          fingerprint: fpState.fingerprint,
          // Daemon doesn't expose this device's own name via get_own_fingerprint;
          // fall back to "This Mac" display name (shown via badge in the row).
          name: "This Mac",
          added_at: 0, // own device — no "paired" date concept
          address: null,
          sync_key_b64: null,
        }
      : null;

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
            className="rounded-ide border border-ide-danger/50 bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-danger hover:bg-ide-hover"
          >
            Yes
          </button>
          <button
            onClick={() => setRevokeAllConfirm(false)}
            className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-dim hover:bg-ide-hover"
          >
            No
          </button>
        </span>
      ) : (
        <button
          onClick={() => setRevokeAllConfirm(true)}
          disabled={revokeAllPending || loadState !== "ready" || peers.length === 0}
          className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-danger hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
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
        <div className="flex items-center gap-3">
          <p className="text-[13px] text-ide-dim">Daemon not running.</p>
          <RestartDaemonButton onRestarted={() => void loadPeers()} />
        </div>
      </ViewShell>
    );
  }

  // --- Degraded state (daemon up, DB unavailable) ---
  // Previously fell through silently; now surfaces a message and restart
  // button so the user can recover without knowing the internals (V-14).
  if (loadState === "degraded") {
    return (
      <ViewShell title="Devices" actions={actions}>
        <div className="flex items-center gap-3">
          <p className="text-[13px] text-ide-warning">
            Database degraded — device list unavailable. Reset the database in
            History to recover.
          </p>
          <RestartDaemonButton onRestarted={() => void loadPeers()} />
        </div>
      </ViewShell>
    );
  }

  // --- Generic error state ---
  // Previously had no early-return branch; render an explicit message and
  // restart button so the user is never left staring at a blank view (V-14).
  if (loadState === "error") {
    return (
      <ViewShell title="Devices" actions={actions}>
        <div className="flex items-center gap-3">
          <p className="text-[13px] text-ide-danger">
            Failed to load devices. Try restarting the daemon.
          </p>
          <RestartDaemonButton onRestarted={() => void loadPeers()} />
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
        {fpState.status === "loading" && (
          <div className="px-3 py-2.5">
            <p className="text-[13px] text-ide-faint animate-pulse">Loading…</p>
          </div>
        )}
        {fpState.status === "offline" && (
          <div className="px-3 py-2.5">
            <p className="text-[13px] text-ide-danger">Daemon not running.</p>
          </div>
        )}
        {/* degraded: daemon is up but DB is unavailable — fingerprint cannot be
            read. Show a placeholder so the row is never silently empty (V-14). */}
        {fpState.status === "degraded" && (
          <div className="px-3 py-2.5">
            <p className="text-[13px] text-ide-warning">
              This device — Unavailable (database degraded)
            </p>
          </div>
        )}
        {thisDevicePeer !== null && (
          <DeviceRow
            peer={thisDevicePeer}
            isThisDevice={true}
            rowSt={undefined}
            onUnpair={() => undefined}
            onRevoke={() => undefined}
            ownFingerprint={ownFingerprint}
            fpCopied={copied}
            onCopyFingerprint={handleCopy}
          />
        )}

        {/* Paired peers */}
        {loadState === "loading" && (
          <div className="px-3 py-2.5">
            <p className="text-[13px] text-ide-faint animate-pulse">Loading peers…</p>
          </div>
        )}
        {loadState === "ready" && peers.length === 0 && (
          <div className="px-3 py-2.5">
            <p className="text-[13px] text-ide-dim">No paired devices yet.</p>
          </div>
        )}
        {loadState === "ready" &&
          peers
            // Don't render this device's own fingerprint a second time if it
            // somehow appears in the peers list (defensive guard).
            .filter((p) => ownFingerprint === null || p.fingerprint !== ownFingerprint)
            .map((peer) => (
              <DeviceRow
                key={peer.fingerprint}
                peer={peer}
                isThisDevice={false}
                rowSt={rowState[peer.fingerprint]}
                onUnpair={(fp) => void handleUnpair(fp)}
                onRevoke={(fp) => void handleRevoke(fp)}
                ownFingerprint={ownFingerprint}
                fpCopied={false}
                onCopyFingerprint={() => undefined}
              />
            ))}
      </div>

      {/* ── Divider ────────────────────────────────────────────── */}
      <div className="my-5 border-t border-ide-divider" />

      {/* ── Pair via QR — full width, compact code ───────────────── */}
      <p className="mb-2 text-[11px] font-medium uppercase tracking-wider text-ide-faint">
        Pair a new device
      </p>

      <section className="rounded-ide border border-ide-border bg-ide-panel p-4 space-y-3">
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
          className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-text hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
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
