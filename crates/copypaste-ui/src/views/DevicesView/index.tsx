// DevicesView — shell after CopyPaste-g06m.15 split.
// Extracted sub-components: SasPairingModal, DiscoveredRow, RevokeConfirmDialog.
// Extracted hooks: useOwnDevice, usePairedDevices, useDiscoveredDevices, useQrCode.
// Default export + props unchanged; all data-testids preserved.
import { useState, useEffect, useCallback } from "react";
import { ConfirmModal } from "../../components/ConfirmModal";
import { Briefcase, RefreshCw, Zap, AlertCircle } from "lucide-react";
// 5917.19: GlassToast system — routes "Revoke all" confirmation/feedback through
// the shared toast provider instead of a raw <span> in the actions bar.
import { useToast, ToastProvider } from "../../components/Toast";
import {
  api,
  ipcErrorMessage,
  IpcError,
  type DiscoveredDevice,
  type PairSasStatus,
} from "../../lib/ipc";
import { usePeerPresence } from "../../lib/peerPresence";
import { ViewShell } from "../../components/ViewShell";
import { RestartDaemonButton } from "../../components/RestartDaemonButton";
import { EmptyState } from "../../components/EmptyState";
import {
  StatusDot,
  ThisDeviceCard,
  PeerRow,
} from "../../components/DeviceCard";
import { SectionHeader } from "../../components/SectionHeader";

import { SasPairingModal } from "./SasPairingModal";
import { DiscoveredRow } from "./DiscoveredRow";
import { RevokeConfirmDialog } from "./RevokeConfirmDialog";
import { useOwnDevice } from "./hooks/useOwnDevice";
import { usePairedDevices } from "./hooks/usePairedDevices";
import { useDiscoveredDevices } from "./hooks/useDiscoveredDevices";
import { useQrCode, QR_TTL_SECS } from "./hooks/useQrCode";

// ---------------------------------------------------------------------------
// Main view (inner — requires ToastProvider ancestor)
// ---------------------------------------------------------------------------

function DevicesViewInner({
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
  // 5917.34: Updated ~every 5 s (30 s when idle) by `App.tsx`'s
  // `startPeerPresencePolling()` loop (POLL_INTERVAL_MS = 5_000).
  // Used as a higher-frequency overlay on top of the 10 s `list_peers` poll so
  // online dots change within ~5 s of a real connect/disconnect.
  const presenceOnline = usePeerPresence((s) => s.online);

  // --- Own device info ---
  const { ownState, ownFpRef } = useOwnDevice();

  // A-4: 1 s clock tick so "last seen Xm ago" labels update live between
  // the 10 s loadPeers polls. Stored as epoch-seconds so PeerRow can compute
  // the elapsed offset without closing over a stale snapshot.
  const [nowSecs, setNowSecs] = useState(() => Math.floor(Date.now() / 1000));
  useEffect(() => {
    const id = setInterval(() => {
      setNowSecs(Math.floor(Date.now() / 1000));
    }, 1000);
    return () => { clearInterval(id); };
  }, []);

  // 5917.19: feedback routed through GlassToast instead of a raw <span>.
  const { show: showToast } = useToast();

  // --- Paired peers ---
  const {
    loadState,
    peers,
    rowState,
    revokeAllPending,
    revokeAllConfirm,
    setRevokeAllConfirm,
    revokePrompt,
    setRevokePrompt,
    rotatePassphrase,
    setRotatePassphrase,
    revokeBusy,
    peersFetchedAtRef,
    loadPeers,
    handleUnpair,
    handleRevoke,
    handleRevokeAndRotate,
    handleRevokeAllConfirmed,
  } = usePairedDevices({ ownFpRef, onShowToast: showToast });

  // --- LAN discovery + SAS pairing ---
  const {
    discovered,
    discoverError,
    setDiscoverError,
    rescanning,
    loadDiscovered,
    handleRescan,
  } = useDiscoveredDevices({ ownFpRef });

  // The device a SAS pairing modal is currently open for, or null.
  const [pairingDevice, setPairingDevice] = useState<DiscoveredDevice | null>(null);
  // Set true while pair_with_discovered is in flight (before the modal opens).
  const [pairStarting, setPairStarting] = useState(false);

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
  const { qrState, qrSecsLeft, qrBlur, handleQrReveal, handleQrRegenerate } = useQrCode();

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
  }, [pairStarting, pairingDevice, setDiscoverError]);

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

  // --- Actions bar ---
  // 5917.19: globalMsg span removed — feedback now goes through GlassToast (showToast above).
  const actions = (
    <div className="flex items-center gap-2">
      {/* uw45: replaced tiny inline Yes/No with a proper modal confirmation */}
      <button
        onClick={() => setRevokeAllConfirm(true)}
        disabled={revokeAllPending || loadState !== "ready" || peers.length === 0}
        className="border border-ide-danger/35 bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-danger hover:bg-ide-raised hover:border-ide-danger/60 shadow-ide-xs disabled:cursor-not-allowed disabled:opacity-40"
        style={{ borderRadius: "var(--skin-r-ctl)" }}
      >
        {revokeAllPending ? "Revoking…" : "Revoke all"}
      </button>
      <ConfirmModal
        open={revokeAllConfirm}
        title="Revoke all paired devices?"
        body={
          <>
            <p>This will immediately break trust with all paired devices.</p>
            <p className="mt-1">All devices will need to re-pair before syncing can resume.</p>
          </>
        }
        confirmLabel="Revoke all"
        busy={revokeAllPending}
        onConfirm={() => {
          setRevokeAllConfirm(false);
          void handleRevokeAllConfirmed();
        }}
        onCancel={() => setRevokeAllConfirm(false)}
      />
    </div>
  );

  // --- Loading state (bdac.2) ---
  // Early-return spinner prevents the empty-layout flash while peers are being
  // fetched. Without this, loadState==="loading" fell through to the main JSX
  // and rendered a blank structured layout indistinguishable from "no devices".
  if (loadState === "loading") {
    return (
      <ViewShell title="Devices" actions={actions}>
        <div className="flex h-full items-center justify-center">
          <span
            aria-label="Loading devices…"
            className="inline-block h-5 w-5 animate-spin motion-reduce:animate-none rounded-full border-2 border-ide-faint border-t-ide-accent"
          />
        </div>
      </ViewShell>
    );
  }

  // --- Offline state ---
  if (loadState === "offline") {
    return (
      <ViewShell title="Devices" actions={actions}>
        <EmptyState
          className="h-full"
          icon={<Zap width={28} height={28} strokeWidth={1.5} />}
          title="Clipboard service offline"
          body="The clipboard service is not running."
          action={<div className="mt-1"><RestartDaemonButton onRestarted={() => void loadPeers()} /></div>}
        />
      </ViewShell>
    );
  }

  // --- Not-ready state (daemon up, still initializing) ---
  // bdac.84: "initialising" → "initializing" (American English); body uses
  // plain user-facing language rather than "service" jargon.
  if (loadState === "not_ready") {
    return (
      <ViewShell title="Devices" actions={actions}>
        <EmptyState
          className="h-full"
          icon={<Zap width={28} height={28} strokeWidth={1.5} />}
          title="Starting…"
          body="CopyPaste is starting up. Your devices will appear in a moment."
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
          body="Try restarting the clipboard service."
          action={<div className="mt-1"><RestartDaemonButton onRestarted={() => void loadPeers()} /></div>}
        />
      </ViewShell>
    );
  }

  return (
    <ViewShell title="Devices" actions={actions}>
      {/* ── Devices section header — with online count ──────────── */}
      {/* The online count uses the live presence store when available (updated
          ~every 5 s (idle: 30 s) by App.tsx's startPeerPresencePolling() loop);
          falls back to peer.online from the last 10 s list_peers poll when the
          store has no entry yet. */}
      {/* zxv2: SectionHeader replaces raw <p> tag.
          crh3.43: faint removed — PARITY-SPEC §3 canonical colour is text-ide-dim. */}
      <div className="mb-2 flex items-center justify-between">
        {/* bdac.48: sentence case to match other section headers */}
        <SectionHeader label="Paired devices" />
        {loadState === "ready" && peers.length > 0 && (
          <span className="inline-flex items-center gap-1 text-[11px] text-ide-faint">
            <StatusDot online={true} />
            {peers.filter((p) => {
              const live = presenceOnline[p.fingerprint];
              return live !== undefined ? live : p.online === true;
            }).length} online
          </span>
        )}
      </div>

      {/* ── Single unified device list (this Mac first, then peers) ── */}
      {/* surface-card glass: the list container is a frosted layer over the aurora.
          W-C4: radius is skin-driven (--skin-r-card) so quiet/vapor skins adapt. */}
      <div
        className="surface-card flex flex-col divide-y divide-ide-divider"
        style={{ borderRadius: "var(--skin-r-card)" }}
      >
        {/* This device — always first */}
        {ownState.status === "loading" && (
          /* Skeleton matches ThisDeviceCard layout: avatar block + two text rows.
             animate-pulse communicates loading shape without layout jump (CopyPaste-5917.22). */
          <div className="flex items-center gap-3 px-3 py-2.5 animate-pulse" aria-busy="true" aria-label="Loading device…">
            <div className="shrink-0 w-[38px] h-[38px] rounded-xl bg-ide-divider" />
            <div className="min-w-0 flex-1 space-y-1.5">
              <div className="h-[13px] w-32 rounded bg-ide-divider" />
              <div className="h-[11px] w-20 rounded bg-ide-divider" />
            </div>
          </div>
        )}
        {ownState.status === "offline" && (
          <div className="px-3 py-2.5">
            {/* bdac.34/36: canonical user-facing term is "Clipboard service" — never "Daemon" */}
            <p className="text-[13px] text-ide-danger">Clipboard service not running.</p>
          </div>
        )}
        {ownState.status === "ready" && (
          <ThisDeviceCard info={ownState.info} />
        )}

        {/* Paired peers — loadState is always "ready" here; "loading" returns
            early with a full-page spinner (CopyPaste-bdac.2). */}
        {loadState === "ready" && peers.length === 0 && (
          <div className="px-3 py-3 flex items-center gap-2">
            {/* Briefcase icon via lucide-react (§9: replace inline SVGs) */}
            <Briefcase size={14} strokeWidth={1.5} className="text-ide-faint shrink-0" />
            <p className="text-[13px] text-ide-dim">No paired devices</p>
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
      {/* zxv2: SectionHeader replaces raw <p> tag. */}
      <div className="mb-2 mt-5 flex items-center justify-between">
        <SectionHeader label="Discovered on your network" />
        <button
          type="button"
          onClick={() => void handleRescan()}
          disabled={rescanning}
          aria-label={rescanning ? "Scanning…" : "Rescan local network"}
          className="flex items-center gap-1 px-2 py-0.5 text-[11px] font-medium text-ide-accent hover:bg-ide-hover disabled:opacity-50 disabled:cursor-default"
          style={{ borderRadius: "var(--skin-r-ctl)" }}
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
        /* W-C4: radius skin-driven (--skin-r-card) */
        <div
          className="surface-card flex flex-col divide-y divide-ide-divider"
          style={{ borderRadius: "var(--skin-r-card)" }}
        >
          {discovered.map((device, idx) => (
            <DiscoveredRow
              key={device.device_id}
              device={device}
              index={idx}
              onPair={(d) => void handlePairDiscovered(d)}
              busy={pairStarting || pairingDevice !== null}
            />
          ))}
        </div>
      ) : (
        // reveal-up: section fades up into view; network-rings on icon = expanding discovery rings
        <div className="reveal-up flex items-center gap-3 py-1">
          <span
            aria-hidden="true"
            className="network-rings shrink-0 inline-flex items-center justify-center w-9 h-9 rounded-xl text-ide-accent"
            style={{
              background: "color-mix(in srgb, var(--accent) 15%, var(--ide-elevated))",
              border: "1px solid color-mix(in srgb, var(--accent) 28%, var(--ide-border))",
            }}
          >
            {/* Wifi-style discovery icon (inline SVG, lucide signal shape) */}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <path d="M5 12.55a11 11 0 0 1 14.08 0" />
              <path d="M1.42 9a16 16 0 0 1 21.16 0" />
              <path d="M8.53 16.11a6 6 0 0 1 6.95 0" />
              <circle cx="12" cy="20" r="1" fill="currentColor" stroke="none" />
            </svg>
          </span>
          <p className="text-[11px] text-ide-faint">No devices found on the network yet.</p>
        </div>
      )}
      {discoverError !== null && (
        <p className="mt-2 text-[11px] text-ide-danger">{discoverError}</p>
      )}

      {/* ── Divider ────────────────────────────────────────────── */}
      <div className="my-5 border-t border-ide-divider" />

      {/* ── Pair via QR — full width, compact code ───────────────── */}
      {/* zxv2: SectionHeader replaces raw <p> tag for consistency */}
      <div className="reveal-up mb-2">
        <SectionHeader label="Pair a new device" />
      </div>

      {/* card-in: glass card entrance (styleguide §device-card).
          W-C4: radius skin-driven; shadow provided by surface-card utility. */}
      <section
        className="card-in surface-card p-4 space-y-3"
        style={{ borderRadius: "var(--skin-r-card)" }}
      >
        {qrState.status === "loading" && (
          // Static muted text — no animate-pulse (MOT-21)
          <p className="text-[12px] text-ide-dim">Generating…</p>
        )}

        {qrState.status === "ready" && (
          <div className="flex items-start gap-5">
            {/* QR code — SVG comes from our own Tauri backend and never
                contains remote markup — dangerouslySetInnerHTML is safe here.
                Privacy-first: .qr-hidden by default (§MO-7 CSS primitive), revealed
                only on intentional click (spec §10 / CopyPaste-1jms.2).
                qrBlur state is INDEPENDENT of QR generation — regenerating the token
                does NOT reset to hidden so the user's reveal decision is preserved. */}
            {/* 5917.85: framed/card treatment — surface-card token supplies
                border + background consistent with other QR surfaces. bg-white
                is preserved inside so the QR module is always black-on-white. */}
            <div
              className={[
                "relative shrink-0 surface-card p-2 overflow-hidden",
                qrBlur === "blurred" ? "qr-hidden" : "qr-visible",
              ].join(" ")}
              style={{ width: 190, height: 190, borderRadius: "var(--skin-r-card)" }}
            >
              {/* qr-grid: the blurred/revealed target (§MO-7) */}
              <div
                className="qr-grid [&>svg]:block [&>svg]:h-full [&>svg]:w-full"
                style={{ width: "100%", height: "100%" }}
                // eslint-disable-next-line react/no-danger
                dangerouslySetInnerHTML={{ __html: qrState.qr.svg }}
              />
              {/* qr-overlay: backdrop-blur frosted reveal affordance (§MO-7).
                  Shown only when blurred; fades out on reveal via CSS transition. */}
              {qrBlur === "blurred" && (
                <button
                  type="button"
                  onClick={handleQrReveal}
                  className="qr-overlay absolute inset-0 flex flex-col items-center justify-center gap-1.5 bg-ide-elevated/10 focus:outline-none focus-visible:ring-2 focus-visible:ring-ide-accent"
                  style={{ borderRadius: "inherit" }}
                  aria-label="Click to reveal QR code"
                >
                  <span className="text-[11px] font-medium text-ide-dim select-none">
                    Click to reveal
                  </span>
                </button>
              )}
            </div>
            <div className="min-w-0 flex-1 space-y-2">
              {/* CopyPaste-1jms.5: The raw QR payload string (CPPAIR2.* — contains
                  PAKE password, device cert fingerprint, Supabase anon key) must
                  NEVER be rendered into the DOM, even when the QR is revealed.
                  The QR SVG above is the sole canonical pairing channel.
                  Rendering the payload as text (even with userSelect:none) left it
                  accessible via element.textContent / browser extensions / a11y APIs.
                  Fix: remove the <p> block entirely. */}
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
                          // progress-pulse: no-op stub (brightness pulse removed, MOT-7); class kept for selector compat
                          className={[
                            "progress-pulse h-full rounded-full transition-[width] duration-1000 ease-linear",
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
          // Static muted text — no animate-pulse (MOT-21)
          <p className="text-[12px] text-ide-dim">Generating pairing code…</p>
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

// ---------------------------------------------------------------------------
// DevicesView — public export wraps the inner component in ToastProvider so
// useToast() calls inside DevicesViewInner have a provider in the tree.
// Pattern mirrors HistoryView (CopyPaste-5917.102 / 5917.19).
// ---------------------------------------------------------------------------

export function DevicesView({
  incomingPairing = null,
}: {
  incomingPairing?: PairSasStatus | null;
} = {}) {
  return (
    <ToastProvider>
      <DevicesViewInner incomingPairing={incomingPairing} />
    </ToastProvider>
  );
}
