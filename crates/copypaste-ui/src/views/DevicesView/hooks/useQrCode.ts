// Extracted from DevicesView.tsx (CopyPaste-g06m.15).
// Cut/paste only — NO behavior changes.
import { useState, useEffect, useCallback, useRef } from "react";
import { pairingQrSvg, type PairingQr } from "../../../lib/ipc";

type QrState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; qr: PairingQr; generatedAt: number }
  | { status: "error"; message: string };

// QR blur is tracked independently of QR generation so regenerating does not
// accidentally clear the privacy blur (spec §10 / CopyPaste-v5a concern).
// Default: blurred (privacy-first). Cleared only when the user explicitly reveals.
type QrBlur = "blurred" | "revealed";

// Pairing token TTL from the daemon (PAKE_SESSION_TTL = 120 s).
// We refresh 15 s before expiry to ensure a valid code is always on-screen.
export const QR_TTL_SECS = 120;
export const QR_REFRESH_MARGIN_SECS = 15;

/**
 * Manages QR pairing code generation, auto-refresh before expiry, privacy blur,
 * and countdown. Visibility-gated: the 1 s tick only runs while the window is
 * in the foreground to avoid burning single-use tokens when hidden.
 */
export function useQrCode() {
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
      // bdac.36: canonical term "clipboard service" — never "daemon" in user-facing strings.
      const next: QrState = { status: "error", message: "Could not generate pairing code. Make sure the clipboard service is running and try again." };
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
    // Re-blur before regenerating: a fresh PAKE session token is a NEW credential
    // and must not be visible without re-confirmation (spec §10 / CopyPaste-crh3.21).
    setQrBlur("blurred");
    void generateQr();
  }, [generateQr]);

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

  return { qrState, qrSecsLeft, qrBlur, handleQrReveal, handleQrRegenerate };
}
