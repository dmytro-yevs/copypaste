// DevicesView — shell after CopyPaste-g06m.15 split.
// Extracted sub-components: SasPairingModal, DiscoveredRow, RevokeConfirmDialog.
// Extracted hooks: useOwnDevice, usePairedDevices, useDiscoveredDevices, useQrCode.
// Default export + props unchanged; all data-testids preserved.
import { useState, useEffect, useCallback } from "react";
import { Briefcase, QrCode, RefreshCw, Trash2, Wifi, X } from "lucide-react";
import { ConfirmModal } from "../../components/ConfirmModal";
import { Dialog } from "../../lib/dialog/Dialog";
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
  onIncomingPairingHandled,
}: {
  /**
   * When the Tauri backend detects an inbound pairing request (responder side),
   * App.tsx passes the `pair_get_sas` payload here so the modal opens regardless
   * of which tab was active when the request arrived.
   */
  incomingPairing?: PairSasStatus | null;
  /**
   * CopyPaste-8ebg.28: App.tsx's `incomingPairing` state was never cleared
   * after DevicesView consumed it into local `responderPairing` state — so
   * closing the tab and switching back to Devices re-seeded a phantom SAS
   * modal from the STALE (already-finished) pairing episode. Call this once
   * the payload has been copied into local state so App.tsx can reset it to
   * `null`, and a later mount only opens the modal for a genuinely new
   * inbound pairing event.
   */
  onIncomingPairingHandled?: () => void;
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
  // after the component is already mounted). CopyPaste-8ebg.28: also tell
  // App.tsx the payload has been consumed so it resets its own
  // `incomingPairing` state to null — otherwise it stays set forever and a
  // later DevicesView mount (e.g. tab away then back) re-seeds
  // `responderPairing` from the same stale, already-finished pairing episode
  // (a phantom SAS modal).
  useEffect(() => {
    if (incomingPairing != null) {
      setResponderPairing(incomingPairing);
      onIncomingPairingHandled?.();
    }
  }, [incomingPairing, onIncomingPairingHandled]);

  // --- QR pairing ---
  // The useQrCode() hook itself keeps its existing eager-generate-on-mount +
  // auto-refresh wiring UNCHANGED (CopyPaste-g27b.19 is presentation-only for
  // this hook) — only the JSX that displays qrState now moves into a modal,
  // gated by this purely-local open flag.
  const { qrState, qrSecsLeft, qrBlur, handleQrReveal, handleQrRegenerate } = useQrCode();
  const [qrModalOpen, setQrModalOpen] = useState(false);

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
      {/* g27b.36a: only one confirm modal may be open at a time — opening
          "Revoke all" while a single-device revoke prompt is open must close
          that one first, otherwise both .modal/.scrim stack on top of each
          other (two coexisting portals). */}
      <button
        type="button"
        className="btn btn--primary sm"
        onClick={() => setQrModalOpen(true)}
      >
        <QrCode aria-hidden="true" />
        Pair a new device
      </button>
      <button
        type="button"
        className="btn btn--danger sm"
        onClick={() => {
          setRevokePrompt(null);
          setRevokeAllConfirm(true);
        }}
        disabled={revokeAllPending || loadState !== "ready" || peers.length === 0}
      >
        <Trash2 aria-hidden="true" />
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
        {/* CopyPaste-8ebg.29: this used to be a classless empty <span> — no
            width/height/background/animation, so it rendered as nothing
            (blank screen indistinguishable from a layout bug) instead of a
            visible loading indicator. `.empty` (centers content, already
            used by EmptyState) + the existing `.spinner` class (primitives.css)
            give it real, already-defined styling. */}
        <div className="empty" aria-busy="true" aria-label="Loading devices…">
          <span className="spinner" aria-hidden="true" />
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
    <div className="dev-scroll">
      {/* ── Devices section header — with online count ──────────── */}
      {/* The online count uses the live presence store when available (updated
          ~every 5 s (idle: 30 s) by App.tsx's startPeerPresencePolling() loop);
          falls back to peer.online from the last 10 s list_peers poll when the
          store has no entry yet. */}
      {/* zxv2: SectionHeader replaces raw <p> tag.
          crh3.43: faint removed — PARITY-SPEC §3 canonical colour is text-ide-dim.
          CopyPaste-g27b.11: this title+count row doubles as the devices view's
          header bar (.dev-head — shell.css) — ViewShell's own <header> is wired
          separately (out of this slice's scope) and stays a plain title above it. */}
      <div className="dev-head">
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
      <div className="dev-list">
        {/* This device — always first */}
        {ownState.status === "loading" && (
          // CopyPaste-8ebg.29: the comment here promised an "animate-pulse"
          // skeleton, but none of these divs ever had a class — they rendered
          // as zero-size invisible boxes (blank screen, not a skeleton).
          // `.dev-hint` + the existing `.spinner` class (primitives.css) are
          // real, already-defined styles, so this is now an actually visible
          // loading row instead of an empty one.
          <div className="dev-hint" aria-busy="true" aria-label="Loading device…">
            <span className="spinner" aria-hidden="true" />
            Loading this device…
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
          <EmptyState
            icon={<Briefcase aria-hidden="true" />}
            title="No paired devices"
            body="Pair your phone or another Mac to sync your clipboard — end-to-end encrypted."
          />
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
                  // g27b.36a: opening the single-device revoke prompt must close
                  // any already-open "Revoke all" confirm so only one confirm
                  // modal is ever on screen at once.
                  setRevokeAllConfirm(false);
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
      {/* zxv2: SectionHeader replaces raw <p> tag.
          g27b.19: .dev-body adds the same 14px/var(--s-6) horizontal inset as
          .dev-head (shell.css) so this header lines up with "Paired devices"
          and the device rows below, instead of sitting flush left. */}
      <div className="dev-subhead dev-body">
        <SectionHeader label="Discovered on your network" />
        <button
          type="button"
          className="btn btn--secondary sm"
          onClick={() => void handleRescan()}
          disabled={rescanning}
          aria-label={rescanning ? "Scanning…" : "Rescan local network"}
          title="Rescan the local network for devices"
        >
          <RefreshCw aria-hidden="true" />
          {rescanning ? "Scanning…" : "Refresh"}
        </button>
      </div>
      {discovered.length > 0 ? (
        <div className="dev-list">
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
        <p className="dev-hint">
          <Wifi aria-hidden="true" />
          No devices found on the network yet.
        </p>
      )}
      {discoverError !== null && (
        <p className="field-note field-note--err dev-body">{discoverError}</p>
      )}
    </div>

      {/* ── QR pairing modal — mounted only while open (CopyPaste-g27b.19).
          Contains the exact same qr-panel content/handlers/hook wiring that
          previously rendered inline; only the JSX's location moved. */}
      {qrModalOpen && (
        <Dialog labelledBy="qr-modal-title" onClose={() => setQrModalOpen(false)}>
          <div className="modal__hd">
            <p id="qr-modal-title" className="modal__t">Pair a new device</p>
            <button
              type="button"
              className="iconbtn"
              aria-label="Close"
              onClick={() => setQrModalOpen(false)}
            >
              <X aria-hidden="true" />
            </button>
          </div>

          <section className="qr-panel">
            {qrState.status === "loading" && (
              // Static muted text — no animate-pulse (MOT-21)
              <p className="field-note">Generating…</p>
            )}

            {qrState.status === "ready" && (
              <>
                {/* QR code — SVG comes from our own Tauri backend and never contains
                    remote markup — dangerouslySetInnerHTML is safe here. Privacy-first:
                    frosted `.qr-reveal` overlay by default, revealed only on intentional
                    click (spec §10 / CopyPaste-1jms.2). qrBlur is INDEPENDENT of QR
                    generation — regenerating the token does NOT reset the reveal state.
                    The `.qr-frame` keeps a white background so the QR is always
                    black-on-white and scannable. */}
                <div className="qr-wrap">
                  <div
                    className="qr-frame"
                    // eslint-disable-next-line react/no-danger
                    dangerouslySetInnerHTML={{ __html: qrState.qr.svg }}
                  />
                  {qrBlur === "blurred" && (
                    <button
                      type="button"
                      className="qr-reveal"
                      onClick={handleQrReveal}
                      aria-label="Click to reveal QR code"
                    >
                      <span>Click to reveal</span>
                    </button>
                  )}
                </div>
                <div className="qr-meta">
                  {/* CopyPaste-1jms.5: The raw QR payload string (CPPAIR2.* — contains
                      PAKE password, device cert fingerprint, Supabase anon key) must
                      NEVER be rendered into the DOM, even when the QR is revealed. The
                      QR SVG above is the sole canonical pairing channel. */}
                  {qrSecsLeft !== null && qrSecsLeft > 0 && (
                    <>
                      {/* Determinate drain bar: width drains from 100% to 0 as the
                          pairing token runs out. CopyPaste-8ebg.15: this used to
                          divide by a stale literal 300 while the real TTL is
                          QR_TTL_SECS (120 s, PAKE_SESSION_TTL) — or whatever the
                          daemon actually returned in expires_in_secs — so the bar
                          started at ~40% and never reached 100%. Use the same TTL
                          basis useQrCode() used to derive qrSecsLeft itself. */}
                      <div className="qr-drain">
                        <div
                          className="qr-drain__fill"
                          data-testid="qr-drain-bar"
                          style={{
                            width: `${Math.max(
                              0,
                              Math.min(
                                100,
                                (qrSecsLeft /
                                  (qrState.status === "ready" && qrState.qr.expires_in_secs > 0
                                    ? qrState.qr.expires_in_secs
                                    : QR_TTL_SECS)) *
                                  100
                              )
                            )}%`,
                          }}
                        />
                      </div>
                      <p className="field-note">
                        Expires in <span>{qrSecsLeft}s</span>
                      </p>
                    </>
                  )}
                  <p className="field-note">
                    Scan from CopyPaste on another device to pair automatically.
                  </p>
                  {/* Explicit regenerate button — separate from reveal so blur state
                      is not accidentally cleared by a refresh (spec §10). */}
                  <button
                    type="button"
                    className="btn btn--secondary sm"
                    onClick={handleQrRegenerate}
                    aria-label="Regenerate pairing QR code"
                  >
                    <RefreshCw aria-hidden="true" />
                    Regenerate
                  </button>
                </div>
              </>
            )}

            {qrState.status === "error" && (
              <p className="field-note field-note--err">{qrState.message}</p>
            )}

            {qrState.status === "idle" && (
              // Static muted text — no animate-pulse (MOT-21)
              <p className="field-note">Generating pairing code…</p>
            )}
          </section>
        </Dialog>
      )}

      {/* ── SAS pairing modal (discovery-initiated, initiator). The responder
          path is handled by the dedicated `responderPairing` modal below,
          driven by the always-on backend poll → "incoming-pairing" event. */}
      {pairingDevice !== null && (
        <SasPairingModal
          device={pairingDevice ?? undefined}
          // CopyPaste-8ebg.28: this modal is the INITIATOR side (opened from
          // `handlePairDiscovered`, not from the inbound "incoming-pairing"
          // event) — it must start from SasPairingModal's own default
          // ({state: "initiating"}), never from `incomingPairing`, which is
          // the RESPONDER-side seed from App.tsx. Passing it here fed a
          // stale/unrelated responder payload into the initiator's modal.
          initialStatus={undefined}
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
  onIncomingPairingHandled,
}: {
  incomingPairing?: PairSasStatus | null;
  onIncomingPairingHandled?: () => void;
} = {}) {
  return (
    <ToastProvider>
      <DevicesViewInner
        incomingPairing={incomingPairing}
        onIncomingPairingHandled={onIncomingPairingHandled}
      />
    </ToastProvider>
  );
}
