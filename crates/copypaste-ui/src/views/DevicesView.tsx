import { useState, useEffect, useCallback, useRef } from "react";
import { api, IpcError, formatWallTime, type PairedDevice } from "../lib/ipc";
import { ViewShell } from "../components/ViewShell";

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
      setRowState({});
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

          return (
            <div
              key={peer.fingerprint}
              className="flex items-center justify-between gap-4 px-3 py-2 hover:bg-ide-hover"
            >
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1.5">
                  <p className="truncate text-[13px] font-medium text-ide-text">
                    {peer.name}
                  </p>
                  {isThisDevice && (
                    <span className="shrink-0 rounded px-1 py-0.5 text-[10px] font-medium bg-ide-accent/15 text-ide-accent">
                      This Mac
                    </span>
                  )}
                </div>
                <p className="truncate font-mono text-[11px] text-ide-dim">
                  {peer.fingerprint}
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
      </div>
    </ViewShell>
  );
}
