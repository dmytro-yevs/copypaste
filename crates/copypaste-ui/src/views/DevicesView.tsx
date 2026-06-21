import { useState, useEffect, useCallback, useRef } from "react";
import { ConfirmModal } from "../components/ConfirmModal";
import { Briefcase, RefreshCw, Zap, AlertCircle } from "lucide-react";
// 5917.19: GlassToast system — routes "Revoke all" confirmation/feedback through
// the shared toast provider instead of a raw <span> in the actions bar.
import { useToast, ToastProvider } from "../components/Toast";
import {
  api,
  ipcErrorMessage,
  IpcError,
  isIpcNotReady,
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
import {
  // DeviceMetaGrid, extractIp used internally by ThisDeviceCard/PeerRow
  StatusDot,
  MetaRow,
  ThisDeviceCard,
  PeerRow,
  type DeviceRowState,
} from "../components/DeviceCard";
import { SectionHeader } from "../components/SectionHeader";

type QrState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; qr: PairingQr; generatedAt: number }
  | { status: "error"; message: string };

// QR blur is tracked independently of QR generation so regenerating does not
// accidentally clear the privacy blur (spec §10 / CopyPaste-v5a concern).
// Default: blurred (privacy-first). Cleared only when the user explicitly reveals.
type QrBlur = "blurred" | "revealed";

// Devices load outcomes. `degraded` (daemon up, DB unavailable), `not_ready`
// (daemon up but still initialising), and `error` (some other failure) are split
// out from `offline` so each condition gets its own friendly message.
type LoadState = "loading" | "offline" | "not_ready" | "degraded" | "error" | "ready";

type OwnDeviceState =
  | { status: "loading" }
  | { status: "ready"; info: OwnDeviceInfo }
  | { status: "offline" };

// DeviceRowState, StatusDot, MetaRow, DeviceMetaGrid, ThisDeviceCard, PeerRow,
// and extractIp are now imported from ../components/DeviceCard above.

// ---------------------------------------------------------------------------
// SAS pairing — discovery-initiated (LAN) pairing modal + discovered list
// ---------------------------------------------------------------------------

/**
 * How often to poll `pair_get_sas` while the SAS pairing modal is open.
 * The daemon's SAS state machine transitions quickly (SAS digits are ready
 * within one round-trip), so 700 ms gives a responsive feel without hammering
 * the IPC socket.
 *
 * Note: if corresponding cadence constants exist in the daemon (copypaste-daemon)
 * or copypaste-p2p crates, keep them in sync manually — there is currently no
 * single shared source of truth for poll intervals across the language boundary.
 * See CopyPaste-x09o for the follow-up tracking item.
 */
const SAS_POLL_MS = 700;
/**
 * How often to refresh the discovered-devices list while the Devices view is open.
 * 3 s is a reasonable balance: fast enough to show a newly-announced peer within
 * a few seconds of it appearing on the LAN, slow enough to avoid busy-looping.
 *
 * Note: mDNS-SD announcement cadence is controlled in copypaste-p2p; if that
 * cadence changes, adjust this constant proportionally so the list stays current.
 */
const DISCOVERED_POLL_MS = 3000;

// bdac.9: watchdog timeout for the SAS pairing modal. If no terminal state
// (confirmed/rejected/aborted/timed_out) is reached within this window, the
// modal shows a friendly error instead of spinning forever. 30 s is generous
// (typical LAN handshake < 5 s) but accommodates slow mDNS discovery.
const SAS_WATCHDOG_MS = 30_000;

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
  // bdac.9: ref holds the watchdog handle so the cleanup callback can cancel it
  // regardless of effect re-runs. Initialised null; set on each poll-effect mount.
  const watchdogRef = useRef<ReturnType<typeof setTimeout> | null>(null);
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
  // modalRef and copiedTimer are declared here; useFocusTrap is called after
  // handleClose is defined below (it depends on handleClose for the onEscape option).
  const modalRef = useRef<HTMLDivElement>(null);

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

    // bdac.9: watchdog — arm once on mount; cleared if a terminal state is
    // reached first. Uses watchdogRef so the cleanup below can always access
    // the latest handle without re-closing over a stale value.
    watchdogRef.current = setTimeout(() => {
      if (!cancelled) {
        setError(
          "Pairing timed out. Check that both devices are on the same network and try again."
        );
      }
    }, SAS_WATCHDOG_MS);

    void poll();
    return () => {
      cancelled = true;
      if (timer !== null) clearTimeout(timer);
      if (watchdogRef.current !== null) {
        clearTimeout(watchdogRef.current);
        watchdogRef.current = null;
      }
    };
  }, [onPaired, initialStatus]);

  // Close the modal. Always resets the daemon pairing state machine via
  // pairAbort — on abort/cancel (non-terminal) this cancels the in-flight
  // pairing; on terminal confirmed/aborted states it resets the machine so a
  // subsequent LAN pairing attempt is not blocked. The daemon handles
  // pair_abort gracefully from any state. (bd CopyPaste-1jms.3, 1jms.12)
  const handleClose = useCallback(() => {
    // Best-effort reset; ignore failure (modal is closing regardless).
    void api.pairAbort().catch(() => {});
    onClose();
  }, [onClose]);

  // Focus trap — traps Tab/Shift+Tab inside the dialog panel and restores focus on close.
  // A11Y-3 / CopyPaste-5917.6: Escape key closes the modal (matching ConfirmModal pattern).
  // A11Y-11 / CopyPaste-5917.30: onEscape wired through useFocusTrap so no separate listener needed.
  // Declared here (after handleClose) because useFocusTrap stores onEscape in a ref internally
  // so the hook always sees the latest value without re-registering the listener.
  useFocusTrap(modalRef, { onEscape: handleClose });

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
          // User said it doesn't match — reset the state machine and close.
          // pairAbort resets to idle so a subsequent pairing is not blocked.
          // (bd CopyPaste-1jms.3)
          void api.pairAbort().catch(() => {});
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
      className="modal-scrim-enter fixed inset-0 z-50 flex items-center justify-center p-6"
      style={{ background: "var(--ide-scrim)" }}
      role="dialog"
      aria-modal="true"
      aria-labelledby="sas-modal-title"
      // A11Y-3 / CopyPaste-5917.6: Escape dismisses; backdrop click cancels
      // (matching the ConfirmModal pattern).
      onClick={(e) => { if (e.target === e.currentTarget) handleClose(); }}
      onKeyDown={(e) => { if (e.key === "Escape") { e.preventDefault(); handleClose(); } }}
    >
      {/* surface-glass-strong = floating frosted-glass pairing dialog.
          modal-card-enter: approved motion entrance (§MO-1).
          W-C4: radius and shadow are skin-driven so quiet/vapor skins adapt. */}
      <div
        ref={modalRef}
        className="modal-card-enter surface-glass-strong w-full max-w-sm p-5"
        style={{ borderRadius: "var(--skin-r-modal)", boxShadow: "var(--skin-shadow-float)" }}
      >
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
            {/* animate-spin + motion-reduce:animate-none — respects reduced-motion (MOT-18) */}
            <span className="inline-block h-3 w-3 animate-spin motion-reduce:animate-none rounded-full border-2 border-ide-faint border-t-ide-accent" />
            <p className="text-[12px] text-ide-dim">Connecting…</p>
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
              {/* animate-spin + motion-reduce:animate-none — respects reduced-motion (MOT-18) */}
              <span className="inline-block h-3 w-3 animate-spin motion-reduce:animate-none rounded-full border-2 border-ide-faint border-t-ide-accent" />
              <p className="text-[12px] text-ide-dim">Waiting for the other device…</p>
            </div>
          )}

        {/* Awaiting SAS — show the code prominently */}
        {!ended && status.state === "awaiting_sas" && status.sas !== undefined && (
          <div className="py-2">
            <p className="mb-2 text-[12px] text-ide-dim">
              Confirm this code matches the one shown on the other device.
            </p>
            {/* Security: SAS code is display-only. userSelect:none + no click-to-copy
                prevents any clipboard reader from grabbing the live PAKE secret.
                (bd CopyPaste-1jms.1) */}
            <div
              className="mx-auto block bg-ide-panel/60 px-4 py-3 font-mono text-[28px] font-semibold tracking-[0.3em] text-ide-text text-center"
              style={{ borderRadius: "var(--skin-r-ctl)", userSelect: "none" }}
              aria-label={`Security code: ${status.sas}`}
              data-testid="sas-code-display"
            >
              {status.sas}
            </div>
            {/* Peer metadata grid — rendered from whatever the daemon knows at
                SAS time (mDNS: name, IPs, fingerprint). All rows are optional;
                nothing renders when a field is absent (responder path). */}
            {/* surface-card: shared glass class instead of raw bg-ide-panel/40 fill (zxv2) */}
            {(status.peer_device_name ??
              status.peer_ip_addrs?.length ??
              status.peer_fingerprint) && (
              <div className="surface-card mt-3 px-3 py-2" style={{ borderRadius: "var(--skin-r-card)" }}>
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
                className="border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-dim hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
                style={{ borderRadius: "var(--skin-r-ctl)" }}
              >
                Doesn't match
              </button>
              {/* wv57: aria-label so screen readers announce "Codes match" even when
                  the visible text is replaced by a spinner while the confirm is in flight.
                  5917.16: use an inline spinner (animate-spin) instead of "..." text so
                  there is a clear loading indicator during the pending state. */}
              <button
                onClick={() => void handleConfirm(true)}
                disabled={confirmPending}
                aria-label="Codes match — confirm pairing"
                className="inline-flex items-center gap-1.5 bg-ide-accent px-3 py-1.5 text-[12px] font-medium text-white hover:bg-ide-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
                style={{ borderRadius: "var(--skin-r-ctl)" }}
              >
                {confirmPending && (
                  <span className="inline-block h-3 w-3 animate-spin motion-reduce:animate-none rounded-full border-2 border-white/40 border-t-white" />
                )}
                {confirmPending ? "Confirming…" : "Match"}
              </button>
            </div>
          </div>
        )}

        {/* Waiting after the user accepted, for the peer to also accept */}
        {!ended && status.state === "awaiting_sas" && status.sas === undefined && (
          <div className="flex items-center gap-2 py-4">
            {/* animate-spin + motion-reduce:animate-none — respects reduced-motion (MOT-18) */}
            <span className="inline-block h-3 w-3 animate-spin motion-reduce:animate-none rounded-full border-2 border-ide-faint border-t-ide-accent" />
            <p className="text-[12px] text-ide-dim">Waiting for the other device…</p>
          </div>
        )}

        {/* Terminal success — use handleClose so pairAbort resets the daemon
            state machine, unblocking a subsequent LAN pairing attempt.
            (bd CopyPaste-1jms.12) */}
        {status.state === "confirmed" && (
          <div className="py-3">
            <p className="text-[13px] font-medium text-ide-success">Paired ✓</p>
            <div className="mt-3 flex justify-end">
              <button
                onClick={handleClose}
                className="border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-text hover:bg-ide-hover"
                style={{ borderRadius: "var(--skin-r-ctl)" }}
              >
                Close
              </button>
            </div>
          </div>
        )}

        {/* Terminal failure — use handleClose so pairAbort resets the daemon
            state machine, unblocking a subsequent LAN pairing attempt.
            (bd CopyPaste-1jms.3) */}
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
                onClick={handleClose}
                className="border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-text hover:bg-ide-hover"
                style={{ borderRadius: "var(--skin-r-ctl)" }}
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
                onClick={handleClose}
                className="border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-text hover:bg-ide-hover"
                style={{ borderRadius: "var(--skin-r-ctl)" }}
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
  index = 0,
}: {
  device: DiscoveredDevice;
  onPair: (device: DiscoveredDevice) => void;
  busy: boolean;
  /** Row index for stagger timing (list-item-in). */
  index?: number;
}) {
  // Show all resolved IPs (comma-joined); fall back to a single address.
  const ips =
    device.ip_addrs.length > 0 ? device.ip_addrs.join(", ") : null;
  // v1 peers without a bootstrap port cannot do SAS pairing.
  const pairable = device.bport !== null;
  return (
    // list-item-in: staggered entrance; stagger delay = index × 60 ms (styleguide §list)
    <div
      className="list-item-in px-3 py-2.5 hover:bg-ide-hover"
      style={{ animationDelay: `${index * 60}ms` }}
    >
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] font-medium text-ide-text">
            {device.device_name || `Device ${device.device_id.slice(0, 8)}`}
          </p>
          <MetaRow label="Addresses" value={ips} />
          {/* PG-43 / CopyPaste-3ese: Android parity — show a visible hint when the
              peer has no bootstrap port (bport=null) so the user knows why Pair is
              disabled, rather than silently greying it out (tooltip-only).
              Matches Android: "This device does not support secure pairing." */}
          {!pairable && (
            <p className="mt-0.5 text-[11px] text-ide-warning">
              This device does not support secure pairing
            </p>
          )}
        </div>
        <button
          onClick={() => onPair(device)}
          disabled={!pairable || busy}
          title={pairable ? undefined : "This device does not support secure pairing"}
          className="shrink-0 bg-ide-accent px-2.5 py-1 text-[12px] font-medium text-white hover:bg-ide-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
          style={{ borderRadius: "var(--skin-r-ctl)" }}
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
  // A11Y-4 / CopyPaste-5917.9: Escape dismisses the revoke dialog.
  // A11Y-11 / CopyPaste-5917.30: routed through useFocusTrap to avoid a separate listener.
  useFocusTrap(dialogRef, { onEscape: onCancel });

  return (
    <div
      className="modal-scrim-enter fixed inset-0 z-50 flex items-center justify-center p-6"
      style={{ background: "var(--ide-scrim)" }}
      role="dialog"
      aria-modal="true"
      aria-labelledby="revoke-modal-title"
      // A11Y-4 / CopyPaste-5917.9: Escape + backdrop click cancel the dialog.
      onClick={(e) => { if (e.target === e.currentTarget) onCancel(); }}
      onKeyDown={(e) => { if (e.key === "Escape") { e.preventDefault(); onCancel(); } }}
    >
      {/* surface-glass-strong = floating frosted-glass revoke dialog.
          modal-card-enter: approved motion entrance (§MO-1).
          W-C4: radius and shadow are skin-driven so quiet/vapor skins adapt. */}
      <div
        ref={dialogRef}
        className="modal-card-enter surface-glass-strong w-full max-w-sm p-5"
        style={{ borderRadius: "var(--skin-r-modal)", boxShadow: "var(--skin-shadow-float)" }}
      >
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
          {/* CopyPaste-5917.25: clarify this field is only used by Revoke & rotate,
              not by the plain Revoke only action. */}
          <span className="ml-1.5 font-normal text-ide-faint/70">— only used by "Revoke &amp; rotate"</span>
        </label>
        <input
          type="password"
          value={rotatePassphrase}
          onChange={(e) => onPassphraseChange(e.target.value)}
          placeholder="At least 8 characters"
          autoComplete="new-password"
          disabled={revokeBusy}
          className="mb-3 w-full border border-ide-border bg-ide-panel/60 px-2.5 py-1.5 text-[12px] text-ide-text placeholder:text-ide-faint focus:border-ide-accent/60 focus:outline-none disabled:opacity-40"
          style={{ borderRadius: "var(--skin-r-ctl)" }}
        />

        <div className="flex items-center justify-end gap-2">
          <button
            onClick={onCancel}
            disabled={revokeBusy}
            className="border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-dim hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
            style={{ borderRadius: "var(--skin-r-ctl)" }}
          >
            Cancel
          </button>
          <button
            onClick={() => onRevoke(fingerprint)}
            disabled={revokeBusy}
            className="border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-danger hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
            style={{ borderRadius: "var(--skin-r-ctl)" }}
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
            // wv57: aria-label is always set so screen readers can identify the
            // action even when the visible text is replaced by "..." when busy.
            aria-label="Revoke and rotate sync key"
            // puf4: solid-danger variant for primary destructive action (Revoke & rotate)
            className="bg-ide-danger px-3 py-1.5 text-[12px] font-medium text-white hover:bg-ide-danger/85 disabled:cursor-not-allowed disabled:opacity-40"
            style={{ borderRadius: "var(--skin-r-ctl)" }}
          >
            {/* bdac.83: aligned to Android label "Revoke & rotate key" for platform parity */}
            {revokeBusy ? "…" : "Revoke & rotate key"}
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
  // 5917.19: feedback routed through GlassToast instead of a raw <span>.
  const { show: showToast } = useToast();

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
      // Log the raw error for diagnostics but NEVER store it in state — it may
      // contain the daemon Unix socket path (/Users/<username>/…) which would
      // leak the local username into the DOM, screen recordings, and the
      // accessibility tree (CopyPaste-tzzu).
      // eslint-disable-next-line no-console
      console.error("[DevicesView] QR generation failed:", err);
      const next: QrState = { status: "error", message: "Could not generate pairing code. Make sure the daemon is running and try again." };
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
      // 1jms.7: immediately zero the countdown when the refresh fires so the
      // displayed countdown accurately reflects the token lifetime. The daemon
      // replaces pending_qr_token the moment generateQr() resolves, so showing
      // "15" while the token is already queued for replacement is misleading.
      if (remaining <= QR_REFRESH_MARGIN_SECS) {
        setQrSecsLeft(0);
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
      // Daemon is up but still initialising its database — show a friendly
      // "starting up" state rather than a hard error.
      if (isIpcNotReady(e)) {
        setLoadState("not_ready");
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
      // Daemon-offline is already surfaced by the peers loader above — stay silent here.
      if (e instanceof IpcError && e.code === "daemon_offline") {
        setDiscovered([]);
        return;
      }
      // All other errors (e.g. P2P disabled, socket timeout) are surfaced inline so
      // the user understands why the discovered list is empty (CopyPaste-44rq.27).
      setDiscovered([]);
      const msg = `Could not load nearby devices: ${ipcErrorMessage(e, "unexpected error")}`;
      setDiscoverError(msg);
      console.error("[DevicesView] loadDiscovered error:", e);
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
        // Log raw error for diagnostics only — never render raw IPC/FS strings
        // in the DOM (CopyPaste-j5qg).
        // eslint-disable-next-line no-console
        console.error("[DevicesView] rescan failed:", e);
        setDiscoverError("Network scan failed. Check that Wi-Fi is on and the daemon is running.");
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
      showToast("Revoked & rotated sync key — re-provision remaining devices", {
        kind: "success",
        duration: 5000,
      });
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
      showToast(`Revoked ${n} device${n === 1 ? "" : "s"}`, { kind: "success", duration: 3000 });
      await loadPeers();
    } catch (err) {
      const msg = ipcErrorMessage(err, "Revoke all failed");
      showToast(msg, { kind: "error", duration: 4000 });
    } finally {
      setRevokeAllPending(false);
    }
  };

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
          body="The daemon is not running."
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
          ~every 5 s (idle: 30 s) by App.tsx's startPeerPresencePolling() loop);
          falls back to peer.online from the last 10 s list_peers poll when the
          store has no entry yet. */}
      {/* zxv2: SectionHeader replaces raw <p> tag; faint=true keeps the
          DevicesView style (text-ide-faint vs SubsectionHeader's text-ide-dim). */}
      <div className="mb-2 flex items-center justify-between">
        {/* bdac.48: sentence case to match other section headers */}
        <SectionHeader label="Paired devices" faint />
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
            <p className="text-[13px] text-ide-danger">Daemon not running.</p>
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
        <SectionHeader label="Discovered on your network" faint />
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
        <SectionHeader label="Pair a new device" faint />
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
                  className="qr-overlay absolute inset-0 flex flex-col items-center justify-center gap-1.5 bg-white/10 focus:outline-none focus-visible:ring-2 focus-visible:ring-ide-accent"
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
