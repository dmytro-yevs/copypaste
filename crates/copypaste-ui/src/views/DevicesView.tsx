import { useState, useEffect, useCallback, useRef } from "react";
import {
  api,
  IpcError,
  formatWallTime,
  pairingQrSvg,
  type PairedDevice,
  type PairingQr,
} from "../lib/ipc";
import { ViewShell } from "../components/ViewShell";

type QrState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; qr: PairingQr }
  | { status: "error"; message: string };

type LoadState = "loading" | "offline" | "ready";

interface DeviceRowState {
  revokedAt: number | null;
  pending: boolean;
  error: string | null;
}

type FingerprintState =
  | { status: "loading" }
  | { status: "ready"; fingerprint: string }
  | { status: "offline" };

export function DevicesView() {
  // --- Devices state ---
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [peers, setPeers] = useState<PairedDevice[]>([]);
  const [rowState, setRowState] = useState<Record<string, DeviceRowState>>({});
  const [revokeAllPending, setRevokeAllPending] = useState(false);
  const [revokeAllConfirm, setRevokeAllConfirm] = useState(false);
  const [globalMsg, setGlobalMsg] = useState<{ text: string; isError: boolean } | null>(null);
  const globalMsgTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // --- Own fingerprint (for labelling this device in the list) ---
  const [fpState, setFpState] = useState<FingerprintState>({ status: "loading" });
  const [copied, setCopied] = useState(false);

  // --- QR pairing (this device displays a code; other devices scan it) ---
  const [qrState, setQrState] = useState<QrState>({ status: "idle" });

  const handleShowQr = useCallback(async () => {
    setQrState({ status: "loading" });
    try {
      const qr = await pairingQrSvg();
      setQrState({ status: "ready", qr });
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to generate pairing code";
      setQrState({ status: "error", message });
    }
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
      setLoadState("ready");
    } catch (e) {
      if (e instanceof IpcError && e.code === "daemon_offline") {
        setLoadState("offline");
      } else {
        setLoadState("offline");
      }
    }
  }, []);

  // --- Load own fingerprint ---
  useEffect(() => {
    let cancelled = false;
    api.getOwnFingerprint().then(
      ({ fingerprint }) => {
        if (!cancelled) setFpState({ status: "ready", fingerprint });
      },
      (err: unknown) => {
        if (cancelled) return;
        const code = err instanceof IpcError ? err.code : null;
        if (code === "daemon_offline") {
          setFpState({ status: "offline" });
        } else {
          setFpState({ status: "offline" });
        }
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
    devicesBody = <p className="text-[13px] text-ide-dim">Daemon not running.</p>;
  } else {
    // Fix #5: filter own device from the peer list in render using the latest
    // ownFingerprint value. loadPeers may complete before getOwnFingerprint,
    // so we guard here rather than relying on load order.
    const remotePeers = ownFingerprint
      ? peers.filter((p) => p.fingerprint !== ownFingerprint)
      : peers;

    if (remotePeers.length === 0) {
      devicesBody = <p className="text-[13px] text-ide-dim">No paired devices.</p>;
    } else {
      devicesBody = (
        <div className="flex flex-col divide-y divide-ide-divider rounded-ide border border-ide-border bg-ide-panel/60">
          {remotePeers.map((peer) => {
            const rs = rowState[peer.fingerprint];
            const isPending = rs?.pending ?? false;
            const revokedAt = rs?.revokedAt ?? null;
            const rowError = rs?.error ?? null;

            return (
              <div
                key={peer.fingerprint}
                className="flex items-center justify-between gap-4 px-3 py-2 hover:bg-ide-hover"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-1.5">
                    <p className="truncate text-[13px] font-medium text-ide-text">
                      {peer.name || `Device ${peer.fingerprint.slice(0, 8)}`}
                    </p>
                  </div>
                  <p
                    className="font-mono text-[11px] text-ide-dim"
                    title={peer.fingerprint}
                  >
                    {peer.fingerprint.length > 32
                      ? `${peer.fingerprint.slice(0, 16)}…${peer.fingerprint.slice(-8)}`
                      : peer.fingerprint}
                  </p>
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
              <span className="text-[13px] text-ide-danger">Daemon not running.</span>
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

          {qrState.status === "idle" && (
            <p className="text-[12px] text-ide-dim">
              Generate a single-use code, then scan it from the CopyPaste app on
              another device to pair automatically — no typing a password.
            </p>
          )}

          {qrState.status === "ready" && (
            <div className="flex flex-col items-center gap-3">
              {/* Fix #10: dangerouslySetInnerHTML is acceptable here. The SVG is
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
              {qrState.qr.expires_in_secs > 0 && (
                <p className="text-[11px] text-ide-dim">
                  Expires in {qrState.qr.expires_in_secs} seconds.
                </p>
              )}
            </div>
          )}

          {qrState.status === "error" && (
            <p className="text-[12px] text-ide-danger">{qrState.message}</p>
          )}

          <button
            type="button"
            onClick={() => void handleShowQr()}
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
      </div>
    </ViewShell>
  );
}
