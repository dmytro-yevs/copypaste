import { useState, useEffect, useCallback } from "react";
import {
  api,
  IpcError,
  formatWallTime,
  formatEpochSecs,
  pairingQrSvg,
  type OwnDeviceInfo,
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
                Paired {formatEpochSecs(peer.added_at!)}
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
  const v6 = address.match(/^\[(.+)\]:\d+$/);
  if (v6) return v6[1];
  const colon = address.lastIndexOf(":");
  if (colon > 0) return address.slice(0, colon);
  return address;
}

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

  // --- QR pairing ---
  const [qrState, setQrState] = useState<QrState>({ status: "idle" });

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

  // --- Load peers ---
  const loadPeers = useCallback(async () => {
    setLoadState("loading");
    try {
      const { peers: fetched } = await api.listPeers();
      const seen = new Set<string>();
      const deduped = fetched.filter((p) => {
        if (seen.has(p.fingerprint)) return false;
        seen.add(p.fingerprint);
        return true;
      });
      const ownFp =
        ownState.status === "ready" ? ownState.info.fingerprint : null;
      // Don't show this device in the peers list.
      const peers = deduped.filter(
        (p) => ownFp === null || p.fingerprint !== ownFp
      );
      setPeers(peers);
      const liveFingerprints = new Set(peers.map((p) => p.fingerprint));
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
      setGlobalMsg({ text: `Revoked ${n} device${n === 1 ? "" : "s"}`, isError: false });
      setTimeout(() => setGlobalMsg(null), 3000);
      await loadPeers();
    } catch (err) {
      const msg = err instanceof IpcError ? err.message : "Revoke all failed";
      setGlobalMsg({ text: msg, isError: true });
      setTimeout(() => setGlobalMsg(null), 4000);
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

  // --- QR ---
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

  // --- Actions bar ---
  const actions = (
    <div className="flex items-center gap-2">
      {globalMsg !== null && (
        <span
          className={["text-[12px]", globalMsg.isError ? "text-ide-danger" : "text-ide-success"].join(" ")}
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

  return (
    <ViewShell title="Devices" actions={actions}>
      {/* ── Section header ────────────────────────────────────── */}
      <p className="mb-2 text-[11px] font-medium uppercase tracking-wider text-ide-faint">
        Devices
      </p>

      {/* ── Unified device list: this device first, then peers ── */}
      <div className="flex flex-col divide-y divide-ide-divider rounded-ide border border-ide-border bg-ide-panel/60">
        {/* This device */}
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
        {loadState === "offline" && (
          <div className="px-3 py-2.5">
            <p className="text-[13px] text-ide-danger">Daemon not running.</p>
          </div>
        )}
        {loadState === "ready" && peers.length === 0 && (
          <div className="px-3 py-2.5">
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

      {/* ── Divider ───────────────────────────────────────────── */}
      <div className="my-5 border-t border-ide-divider" />

      {/* ── Pair via QR ───────────────────────────────────────── */}
      <p className="mb-2 text-[11px] font-medium uppercase tracking-wider text-ide-faint">
        Pair a new device
      </p>

      <section className="rounded-ide border border-ide-border bg-ide-panel p-4 space-y-3">
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
          <div className="flex flex-col items-center gap-3">
            <div
              className="rounded-ide bg-white p-3 overflow-hidden [&>svg]:block [&>svg]:h-full [&>svg]:w-full"
              style={{ width: 190, height: 190 }}
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
    </ViewShell>
  );
}
