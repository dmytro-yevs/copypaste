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

export function DevicesView() {
  // --- Devices state ---
  const [loadState, setLoadState] = useState<LoadState>("loading");
  // Detail line for the degraded/error states so the failure path is LOUD
  // (the real reason / message, never a blank or mislabeled screen).
  const [loadDetail, setLoadDetail] = useState<string | null>(null);
  const [peers, setPeers] = useState<PairedDevice[]>([]);
  const [rowState, setRowState] = useState<Record<string, DeviceRowState>>({});
  const [revokeAllPending, setRevokeAllPending] = useState(false);
  const [revokeAllConfirm, setRevokeAllConfirm] = useState(false);
  const [globalMsg, setGlobalMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const globalMsgTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // --- Own fingerprint (for labelling this device in the list) ---
  const [fpState, setFpState] = useState<FingerprintState>({ status: "loading" });
  const [copied, setCopied] = useState(false);
  // Incrementing this triggers a fingerprint re-fetch (e.g. after restart).
  const [fpReloadKey, setFpReloadKey] = useState(0);

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
      // Fix #4: preserve transient per-row state (e.g. the "Revoked · <time>"
      // line set by handleRevoke) across reloads. Previously this reset to {},
      // so the revoked confirmation was wiped by the immediate loadPeers() that
      // follows a revoke and never rendered. We keep state only for fingerprints
      // still present in the refreshed peer list so stale rows don't accumulate.
      const liveFingerprints = new Set(deduped.map((p) => p.fingerprint));
      setRowState((prev) => {
        const next: Record<string, DeviceRowState> = {};
        for (const fp of liveFingerprints) {
          if (prev[fp]) next[fp] = prev[fp];
        }
        return next;
      });
      setLoadDetail(null);
      setLoadState("ready");
    } catch (e) {
      // A transport failure means the daemon is genuinely unreachable.
      if (e instanceof IpcError && e.code === "daemon_offline") {
        setLoadDetail(null);
        setLoadState("offline");
        return;
      }
      // The daemon answered but list_peers failed. Probe `status` to tell a
      // DB-degraded daemon (recoverable via History → Reset database) apart from
      // a generic error — previously both fell through to "offline" and the
      // degraded case was mislabeled "Daemon not running."
      const probe = await probeStatus();
      if (probe.kind === "offline") {
        setLoadDetail(null);
        setLoadState("offline");
      } else if (probe.kind === "degraded") {
        setLoadDetail(probe.reason);
        setLoadState("degraded");
      } else {
        setLoadDetail(e instanceof IpcError ? e.message : String(e));
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
  // fpReloadKey lets the RestartDaemonButton trigger a re-fetch after restart.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fpReloadKey]);

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
    if (fpState.status !== "ready") return;
    navigator.clipboard.writeText(fpState.fingerprint).then(
      () => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      },
      () => {
        const el = document.createElement("textarea");
        el.value = fpState.fingerprint;
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

  // --- Actions bar (top-right of ViewShell) ---
  const ownFingerprint =
    fpState.status === "ready" ? fpState.fingerprint : null;

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

  // --- Devices list body ---
  let devicesBody: React.ReactNode;

  if (loadState === "loading") {
    devicesBody = <p className="text-[13px] text-ide-dim">Loading…</p>;
  } else if (loadState === "offline") {
    devicesBody = (
      <div className="flex items-center gap-3">
        <p className="text-[13px] text-ide-dim">Daemon not running.</p>
        <RestartDaemonButton onRestarted={() => void loadPeers()} />
      </div>
    );
  } else if (loadState === "degraded") {
    devicesBody = (
      <div className="rounded-ide border border-ide-warning/40 bg-ide-warning/5 px-3 py-2.5 text-[13px] text-ide-warning">
        <p className="font-medium">Clipboard database unavailable</p>
        <p className="mt-1 text-[12px] text-ide-dim">
          The daemon is running but its encrypted database could not be opened
          {loadDetail ? ` (${loadDetail})` : ""}. Device pairing is paused until
          it recovers. Open History to reset the database.
        </p>
      </div>
    );
  } else if (loadState === "error") {
    devicesBody = (
      <div className="rounded-ide border border-ide-danger/40 bg-ide-panel px-3 py-2.5 text-[13px] text-ide-danger">
        <p className="font-medium">Couldn't load devices</p>
        {loadDetail && (
          <p className="mt-1 break-words text-[12px] text-ide-dim">{loadDetail}</p>
        )}
      </div>
    );
  } else if (peers.length === 0) {
    devicesBody = <p className="text-[13px] text-ide-dim">No paired devices.</p>;
  } else {
    devicesBody = (
      <div className="flex flex-col divide-y divide-ide-divider rounded-ide border border-ide-border bg-ide-panel/60">
        {peers.map((peer) => {
          const rs = rowState[peer.fingerprint];
          const isPending = rs?.pending ?? false;
          const revokedAt = rs?.revokedAt ?? null;
          const rowError = rs?.error ?? null;
          // Label this device if its fingerprint matches our own.
          const isThisDevice =
            ownFingerprint !== null && peer.fingerprint === ownFingerprint;

          // Extract host from "host:port" address if present.
          const peerHost = peer.address
            ? peer.address.replace(/:\d+$/, "")
            : null;

          return (
            <div
              key={peer.fingerprint}
              className="flex items-start justify-between gap-4 px-3 py-2.5 hover:bg-ide-hover"
            >
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1.5">
                  <p className="truncate text-[13px] font-medium text-ide-text">
                    {/* Fix #8: friendly label — use peer.name; show short fingerprint prefix if blank */}
                    {peer.name || `Device ${peer.fingerprint.slice(0, 8)}`}
                  </p>
                  {isThisDevice && (
                    <span className="shrink-0 rounded px-1 py-0.5 text-[10px] font-medium bg-ide-accent/15 text-ide-accent">
                      This Mac
                    </span>
                  )}
                </div>

                {/* Fix #8: truncated fingerprint so the row stays compact */}
                <p
                  className="font-mono text-[11px] text-ide-dim"
                  title={peer.fingerprint}
                >
                  {peer.fingerprint.length > 32
                    ? `${peer.fingerprint.slice(0, 16)}…${peer.fingerprint.slice(-8)}`
                    : peer.fingerprint}
                </p>

                {/* Rich device metadata — only shown when data is available */}
                <div className="mt-0.5 flex flex-wrap gap-x-3 gap-y-0.5">
                  {peerHost && (
                    <span className="text-[11px] text-ide-faint">
                      <span className="text-ide-dim">IP</span>{" "}
                      <span className="font-mono">{peerHost}</span>
                    </span>
                  )}
                  {peer.added_at != null && peer.added_at > 0 && (
                    <span className="text-[11px] text-ide-faint">
                      <span className="text-ide-dim">Paired</span>{" "}
                      {formatEpochSecs(peer.added_at)}
                    </span>
                  )}
                </div>

                {revokedAt !== null && (
                  <p className="text-[11px] text-ide-accent">
                    Revoked · {formatWallTime(revokedAt)}
                  </p>
                )}
                {rowError !== null && (
                  <p className="text-[11px] text-ide-danger">{rowError}</p>
                )}
              </div>
              <div className="flex shrink-0 items-center gap-1.5">
                <button
                  onClick={() => void handleUnpair(peer.fingerprint)}
                  disabled={isPending}
                  className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-text hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
                >
                  {isPending ? "…" : "Unpair"}
                </button>
                <button
                  onClick={() => void handleRevoke(peer.fingerprint)}
                  disabled={isPending}
                  className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-danger hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
                >
                  {isPending ? "…" : "Revoke"}
                </button>
              </div>
            </div>
          );
        })}
      </div>
    );
  }

  return (
    <ViewShell title="Devices" actions={actions}>
      {/* ── Paired devices ─────────────────────────────────────── */}
      {devicesBody}

      {/* ── Divider ────────────────────────────────────────────── */}
      <div className="my-5 border-t border-ide-divider" />

      {/* ── This device's fingerprint (info-only, no pairing form) ── */}
      <p className="mb-3 text-[11px] font-medium uppercase tracking-wider text-ide-faint">
        This device
      </p>

      <div className="mx-auto max-w-md">
        <section className="rounded-ide border border-ide-border bg-ide-panel p-4 space-y-2">
          <p className="text-[11px] font-medium uppercase tracking-wider text-ide-faint">
            Fingerprint
          </p>
          <div className="flex items-center gap-2">
            {fpState.status === "loading" && (
              <span className="font-mono text-[13px] text-ide-faint animate-pulse">
                Loading…
              </span>
            )}
            {fpState.status === "offline" && (
              <>
                <span className="text-[13px] text-ide-danger">Daemon not running.</span>
                <RestartDaemonButton
                  onRestarted={() => {
                    setFpReloadKey((k) => k + 1);
                    void loadPeers();
                  }}
                />
              </>
            )}
            {fpState.status === "degraded" && (
              <span className="text-[13px] text-ide-warning">
                Unavailable — database degraded
                {fpState.reason ? ` (${fpState.reason})` : ""}.
              </span>
            )}
            {fpState.status === "ready" && (
              <>
                <span className="select-all font-mono text-[13px] text-ide-text break-all">
                  {fpState.fingerprint}
                </span>
                <button
                  type="button"
                  onClick={handleCopy}
                  className="shrink-0 rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-dim hover:bg-ide-hover hover:text-ide-text transition-colors"
                >
                  {copied ? "Copied" : "Copy"}
                </button>
              </>
            )}
          </div>
          <p className="text-[11px] text-ide-faint">
            Share this fingerprint with another device to identify this Mac.
          </p>
        </section>

        {/* ── Pair via QR ──────────────────────────────────────── */}
        <section className="mt-4 rounded-ide border border-ide-border bg-ide-panel p-4 space-y-3">
          <p className="text-[11px] font-medium uppercase tracking-wider text-ide-faint">
            Pair via QR
          </p>

          {qrState.status === "loading" && (
            <p className="text-[12px] text-ide-dim animate-pulse">Generating…</p>
          )}

          {qrState.status === "ready" && (
            <div className="flex flex-col items-center gap-3">
              {/* dangerouslySetInnerHTML is acceptable here. The SVG is
                  produced entirely by our own Tauri backend (ipc.rs::render_svg
                  via the `qrcode` crate) from a pairing payload — it never
                  contains remote, daemon-supplied, or user-entered markup, so
                  there is no XSS surface. Rendering as inline SVG (rather than a
                  data-URI <img>) keeps the QR crisp at any DPI. */}
              <div
                className="rounded-ide bg-white p-3"
                // eslint-disable-next-line react/no-danger
                dangerouslySetInnerHTML={{ __html: qrState.qr.svg }}
              />
              <p className="select-all break-all text-center font-mono text-[10px] text-ide-faint">
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
                : "Generate code"}
          </button>
        </section>
      </div>
    </ViewShell>
  );
}
