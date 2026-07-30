/**
 * Decoding is `@zxing/browser`, pure JavaScript rather than the WASM
 * ponyfills, which fetch their binary from a CDN by default and would be the
 * one thing in this app that needs the network to pair two devices on a LAN.
 *
 * **It is allowed to be unavailable.** No camera, a refused permission and no
 * secure context all resolve to the same state: the dialog falls back to
 * typing. Pairing must never depend on a camera.
 *
 * **Nothing scanned is trusted.** A decode is a *string a camera saw*, so it
 * goes through `decodePairing` before it reaches the caller — a Wi-Fi QR or a
 * poster in the background cannot become a pairing attempt.
 */
import { useEffect, useRef, useState } from "react";
import { BrowserQRCodeReader } from "@zxing/browser";
import { CameraOff } from "lucide-react";

import { useTranslation } from "@/i18n";
import { type PairingPayload, decodePairing } from "@/lib/pairing";

type Phase = "starting" | "scanning" | "unavailable";

interface QrScannerProps {
  /** Called once, with a payload that has already been validated. */
  onScan: (payload: PairingPayload) => void;
}

export function QrScanner({ onScan }: QrScannerProps) {
  const { t } = useTranslation();
  const videoRef = useRef<HTMLVideoElement>(null);
  const [phase, setPhase] = useState<Phase>("starting");
  // Held in a ref so the effect does not re-run when it changes, and so a
  // second decode of the same frame cannot fire the callback twice.
  const doneRef = useRef(false);
  const onScanRef = useRef(onScan);
  onScanRef.current = onScan;

  useEffect(() => {
    const video = videoRef.current;
    if (!video || typeof navigator === "undefined" || !navigator.mediaDevices) {
      setPhase("unavailable");
      return;
    }

    let stop: (() => void) | undefined;
    let cancelled = false;
    doneRef.current = false;

    const reader = new BrowserQRCodeReader();
    reader
      .decodeFromVideoDevice(undefined, video, (result) => {
        if (!result || doneRef.current) return;
        const payload = decodePairing(result.getText());
        // A QR that is not ours is not an error: the camera is pointed at a
        // room. Keep scanning.
        if (!payload) return;
        doneRef.current = true;
        onScanRef.current(payload);
      })
      .then((controls) => {
        if (cancelled) {
          controls.stop();
          return;
        }
        stop = () => controls.stop();
        setPhase("scanning");
      })
      .catch(() => {
        // No camera, no permission, or no secure context. One state for all
        // three: the user's next action is the same.
        if (!cancelled) setPhase("unavailable");
      });

    return () => {
      cancelled = true;
      // Releasing the camera matters: a stream left running keeps the
      // recording indicator lit after the dialog has closed, which for a
      // clipboard manager reads exactly as badly as it sounds.
      stop?.();
    };
  }, []);

  if (phase === "unavailable") {
    return (
      <div className="flex flex-col items-center gap-s-2 rounded-md border border-divider bg-raised-1 px-s-3 py-s-3 text-center">
        <CameraOff size={18} aria-hidden="true" className="text-muted-foreground" />
        <p className="text-xs text-muted-foreground">
          {t("devices.scanner.unavailable")}
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-col items-center gap-s-2">
      <video
        ref={videoRef}
        // Muted and inline or a mobile WebView takes the video fullscreen.
        muted
        playsInline
        aria-label={t("devices.scanner.videoLabel")}
        className="aspect-square w-full max-w-[240px] rounded-md bg-black object-cover"
      />
      <p aria-live="polite" className="text-xs text-muted-foreground">
        {t(
          phase === "starting"
            ? "devices.scanner.starting"
            : "devices.scanner.scanning",
        )}
      </p>
    </div>
  );
}
