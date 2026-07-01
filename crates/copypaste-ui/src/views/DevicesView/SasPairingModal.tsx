// Extracted from DevicesView.tsx (CopyPaste-g06m.15).
// Cut/paste only — NO behavior changes.
import { useState, useEffect, useCallback, useRef } from "react";
import { MetaRow } from "../../components/DeviceCard";
import {
  api,
  ipcErrorMessage,
  type DiscoveredDevice,
  type PairSasStatus,
} from "../../lib/ipc";
import { Dialog } from "../../lib/dialog/Dialog";

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
export const SAS_POLL_MS = 700;

// bdac.9: watchdog timeout for the SAS pairing modal. If no terminal state
// (confirmed/rejected/aborted/timed_out) is reached within this window, the
// modal shows a friendly error instead of spinning forever. 30 s is generous
// (typical LAN handshake < 5 s) but accommodates slow mDNS discovery.
export const SAS_WATCHDOG_MS = 30_000;

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
export function SasPairingModal({
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

  // Focus trap + Escape/backdrop dismissal now come from the shared Dialog
  // primitive (task 2.9); onClose=handleClose preserves the pairAbort-on-close
  // behavior (A11Y-3/A11Y-11).

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
    <Dialog labelledBy="sas-modal-title" onClose={handleClose}>
        <p id="sas-modal-title" className="modal__t">
          {isResponder ? `"${peerName}" wants to pair` : `Pair "${peerName}"`}
        </p>

        {/* Peer metadata (responder path, or when daemon provides it).
            CopyPaste-g27b.11: MetaRow now renders a .cfield button, so its
            container needs the .cfields grid (patterns.css) to lay out right. */}
        {(status.peer_model || status.peer_os || status.peer_app_version || status.peer_ip) && (
          <div className="cfields">
            <MetaRow label="Model" value={status.peer_model} />
            <MetaRow label="OS" value={status.peer_os} />
            <MetaRow label="Version" value={status.peer_app_version} />
            <MetaRow label="IP" value={status.peer_ip} />
          </div>
        )}

        {/* Connecting / initiating */}
        {!ended && status.state === "initiating" && error === null && (
          <div>
            {/* animate-spin + motion-reduce:animate-none — respects reduced-motion (MOT-18) */}
            <span />
            <p>Connecting…</p>
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
            <div>
              {/* animate-spin + motion-reduce:animate-none — respects reduced-motion (MOT-18) */}
              <span />
              <p>Waiting for the other device…</p>
            </div>
          )}

        {/* Awaiting SAS — show the code prominently */}
        {!ended && status.state === "awaiting_sas" && status.sas !== undefined && (
          <div>
            <p>
              Confirm this code matches the one shown on the other device.
            </p>
            {/* Security: SAS code is display-only. userSelect:none + no click-to-copy
                prevents any clipboard reader from grabbing the live PAKE secret.
                (bd CopyPaste-1jms.1) CopyPaste-g27b.11: each digit is its own
                .sas span (patterns.css); the 6-digit string itself is unchanged. */}
            <div
              className="sas"
              aria-label={`Security code: ${status.sas}`}
              data-testid="sas-code-display"
            >
              {status.sas.split("").map((digit, i) => (
                <span key={i}>{digit}</span>
              ))}
            </div>
            {/* Peer metadata grid — rendered from whatever the daemon knows at
                SAS time (mDNS: name, IPs, fingerprint). All rows are optional;
                nothing renders when a field is absent (responder path). */}
            {/* surface-card: shared glass class instead of raw bg-ide-panel/40 fill (zxv2) */}
            {(status.peer_device_name ??
              status.peer_ip_addrs?.length ??
              status.peer_fingerprint) && (
              <div className="cfields">
                <MetaRow label="Name" value={status.peer_device_name} />
                <MetaRow
                  label="Addresses"
                  value={status.peer_ip_addrs?.join(", ")}
                />
                <MetaRow label="Fingerprint" value={status.peer_fingerprint} />
              </div>
            )}
            <div className="modal__act">
              <button
                type="button"
                className="btn btn--secondary"
                onClick={() => void handleConfirm(false)}
                disabled={confirmPending}
              >
                Doesn't match
              </button>
              {/* wv57: aria-label so screen readers announce "Codes match" even when
                  the visible text is replaced by a spinner while the confirm is in flight.
                  5917.16: use an inline spinner (animate-spin) instead of "..." text so
                  there is a clear loading indicator during the pending state. */}
              <button
                type="button"
                className="btn btn--primary"
                onClick={() => void handleConfirm(true)}
                disabled={confirmPending}
                aria-label="Codes match — confirm pairing"
              >
                {confirmPending && (
                  <span className="spinner" />
                )}
                {confirmPending ? "Confirming…" : "Match"}
              </button>
            </div>
          </div>
        )}

        {/* Waiting after the user accepted, for the peer to also accept */}
        {!ended && status.state === "awaiting_sas" && status.sas === undefined && (
          <div>
            {/* animate-spin + motion-reduce:animate-none — respects reduced-motion (MOT-18) */}
            <span />
            <p>Waiting for the other device…</p>
          </div>
        )}

        {/* Terminal success — use handleClose so pairAbort resets the daemon
            state machine, unblocking a subsequent LAN pairing attempt.
            (bd CopyPaste-1jms.12) */}
        {status.state === "confirmed" && (
          <div>
            <p>Paired ✓</p>
            <div className="modal__act">
              <button type="button" className="btn btn--secondary" onClick={handleClose}>
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
          <div>
            <p>
              {status.state === "timed_out"
                ? "Pairing timed out."
                : status.state === "rejected"
                  ? "Pairing was rejected."
                  : "Pairing was cancelled."}
            </p>
            <div className="modal__act">
              <button type="button" className="btn btn--secondary" onClick={handleClose}>
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
          <div>
            <p>
              Pairing ended — check the other device.
            </p>
            <div className="modal__act">
              <button type="button" className="btn btn--secondary" onClick={handleClose}>
                Close
              </button>
            </div>
          </div>
        )}

        {/* Transient poll/confirm error (non-terminal) */}
        {error !== null && !terminal && (
          <p>{error}</p>
        )}

        {/* Cancel affordance for the non-terminal states (Connecting / awaiting) */}
        {!terminal && (
          <div className="modal__act">
            <button type="button" className="btn btn--secondary" onClick={handleClose}>
              Cancel
            </button>
          </div>
        )}
    </Dialog>
  );
}
