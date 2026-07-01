// DevicesView — shell after CopyPaste-g06m.15 split.
// Extracted sub-components: SasPairingModal, DiscoveredRow, RevokeConfirmDialog.
// Extracted hooks: useOwnDevice, usePairedDevices, useDiscoveredDevices, useQrCode.
// Default export + props unchanged; all data-testids preserved.
import { useState, useEffect, useCallback } from "react";
import { ConfirmModal } from "../../components/ConfirmModal";
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
import { useQrCode } from "./hooks/useQrCode";

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
    <div>
      {/* uw45: replaced tiny inline Yes/No with a proper modal confirmation */}
      <button
        onClick={() => setRevokeAllConfirm(true)}
        disabled={revokeAllPending || loadState !== "ready" || peers.length === 0}
      >
        {revokeAllPending ? "Revoking…" : "Revoke all"}
      </button>
      <ConfirmModal
        open={revokeAllConfirm}
        title="Revoke all paired devices?"
        body={
          <>
            <p>This will immediately break trust with all paired devices.</p>
            <p>All devices will need to re-pair before syncing can resume.</p>
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
        <div>
          <span
            aria-label="Loading devices…"
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
          title="Clipboard service offline"
          body="The clipboard service is not running."
          action={<div><RestartDaemonButton onRestarted={() => void loadPeers()} /></div>}
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
          title="Database degraded"
          body="Device list unavailable. Reset the database in History to recover."
          action={<div><RestartDaemonButton onRestarted={() => void loadPeers()} /></div>}
        />
      </ViewShell>
    );
  }

  // --- Generic error state ---
  if (loadState === "error") {
    return (
      <ViewShell title="Devices" actions={actions}>
        <EmptyState
          title="Failed to load devices"
          body="Try restarting the clipboard service."
          action={<div><RestartDaemonButton onRestarted={() => void loadPeers()} /></div>}
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
      <div>
        {/* bdac.48: sentence case to match other section headers */}
        <SectionHeader label="Paired devices" />
        {loadState === "ready" && peers.length > 0 && (
          <span>
            <StatusDot online={true} />
            {peers.filter((p) => {
              const live = presenceOnline[p.fingerprint];
              return live !== undefined ? live : p.online === true;
            }).length} online
          </span>
        )}
      </div>

      {/* ── Single unified device list (this Mac first, then peers) ── */}
      <div>
        {/* This device — always first */}
        {ownState.status === "loading" && (
          /* Skeleton matches ThisDeviceCard layout: avatar block + two text rows.
             animate-pulse communicates loading shape without layout jump (CopyPaste-5917.22). */
          <div aria-busy="true" aria-label="Loading device…">
            <div />
            <div>
              <div />
              <div />
            </div>
          </div>
        )}
        {ownState.status === "offline" && (
          <div>
            {/* bdac.34/36: canonical user-facing term is "Clipboard service" — never "Daemon" */}
            <p>Clipboard service not running.</p>
          </div>
        )}
        {ownState.status === "ready" && (
          <ThisDeviceCard info={ownState.info} />
        )}

        {/* Paired peers — loadState is always "ready" here; "loading" returns
            early with a full-page spinner (CopyPaste-bdac.2). */}
        {loadState === "ready" && peers.length === 0 && (
          <div>
            {/* Briefcase icon via lucide-react (§9: replace inline SVGs) */}
            <p>No paired devices</p>
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
      <div>
        <SectionHeader label="Discovered on your network" />
        <button
          type="button"
          onClick={() => void handleRescan()}
          disabled={rescanning}
          aria-label={rescanning ? "Scanning…" : "Rescan local network"}
          title="Rescan the local network for devices"
        >
          {/* RefreshCw from lucide-react; spins while rescanning; reduced-motion: static */}
          {rescanning ? "Scanning…" : "Refresh"}
        </button>
      </div>
      {discovered.length > 0 ? (
        <div>
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
        <div>
          <span
            aria-hidden="true"
          />
          <p>No devices found on the network yet.</p>
        </div>
      )}
      {discoverError !== null && (
        <p>{discoverError}</p>
      )}

      {/* ── Divider ────────────────────────────────────────────── */}
      <div />

      {/* ── Pair via QR — full width, compact code ───────────────── */}
      {/* zxv2: SectionHeader replaces raw <p> tag for consistency */}
      <div>
        <SectionHeader label="Pair a new device" />
      </div>

      {/* card-in: glass card entrance (styleguide §device-card). */}
      <section>
        {qrState.status === "loading" && (
          // Static muted text — no animate-pulse (MOT-21)
          <p>Generating…</p>
        )}

        {qrState.status === "ready" && (
          <div>
            {/* QR code — SVG comes from our own Tauri backend and never
                contains remote markup — dangerouslySetInnerHTML is safe here.
                Privacy-first: .qr-hidden by default (§MO-7 CSS primitive), revealed
                only on intentional click (spec §10 / CopyPaste-1jms.2).
                qrBlur state is INDEPENDENT of QR generation — regenerating the token
                does NOT reset to hidden so the user's reveal decision is preserved. */}
            {/* 5917.85: framed/card treatment — surface-card token supplies
                border + background consistent with other QR surfaces. bg-white
                is preserved inside so the QR module is always black-on-white. */}
            <div>
              {/* qr-grid: the blurred/revealed target (§MO-7) */}
              <div
                // eslint-disable-next-line react/no-danger
                dangerouslySetInnerHTML={{ __html: qrState.qr.svg }}
              />
              {/* qr-overlay: backdrop-blur frosted reveal affordance (§MO-7).
                  Shown only when blurred; fades out on reveal via CSS transition. */}
              {qrBlur === "blurred" && (
                <button
                  type="button"
                  onClick={handleQrReveal}
                  aria-label="Click to reveal QR code"
                >
                  <span>
                    Click to reveal
                  </span>
                </button>
              )}
            </div>
            <div>
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
                  <div>
                    <div
                      data-testid="qr-drain-bar"
                      // progress-pulse: no-op stub (brightness pulse removed, MOT-7); class kept for selector compat
                    />
                  </div>
                  <p>
                    Expires in{" "}
                    <span>
                      {qrSecsLeft}s
                    </span>
                  </p>
                </>
              )}
              <p>
                Scan from CopyPaste on another device to pair automatically.
              </p>
              {/* Explicit regenerate button — separate from reveal so blur state
                  is not accidentally cleared by a refresh (spec §10). */}
              <button
                type="button"
                onClick={handleQrRegenerate}
                aria-label="Regenerate pairing QR code"
              >
                Regenerate
              </button>
            </div>
          </div>
        )}

        {qrState.status === "error" && (
          <p>{qrState.message}</p>
        )}

        {qrState.status === "idle" && (
          // Static muted text — no animate-pulse (MOT-21)
          <p>Generating pairing code…</p>
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
