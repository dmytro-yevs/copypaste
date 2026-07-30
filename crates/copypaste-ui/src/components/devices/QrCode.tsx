/**
 * A QR code, drawn to a canvas.
 *
 * **INV-13: the payload string must never enter the DOM.** A canvas is what
 * makes that structural rather than a rule to remember — the pixels are the
 * only representation, so there is no attribute, no text node and no SVG
 * `<path>` holding the credential for a screenshot tool, an accessibility
 * inspector or a devtools copy-as-HTML to pick up. `qrcode`'s SVG renderer
 * would have been fewer lines and would have put the payload in the markup.
 *
 * The value is passed to the canvas and never stored: nothing here keeps a
 * copy, and unmounting takes the drawing with it.
 *
 * Rendering is deliberately not memoised on anything but the value. The dialog
 * that owns this unmounts when it closes, and the QR must go with it.
 */
import { useEffect, useRef, useState } from "react";
import QRCode from "qrcode";

interface QrCodeProps {
  /** The payload. Secret: never rendered as text, never logged. */
  value: string;
  /** Side length in CSS pixels. */
  size?: number;
  /** Described to a screen reader, since the image itself cannot be. */
  label: string;
}

export function QrCode({ value, size = 200, label }: QrCodeProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    let cancelled = false;

    QRCode.toCanvas(canvas, value, {
      width: size,
      margin: 2,
      // Medium recovery: enough to survive a phone camera at an angle without
      // making the modules so small that a low-light scan fails.
      errorCorrectionLevel: "M",
      // Pure black on pure white rather than the theme's colours. A QR is read
      // by a camera, not by a person, and the contrast a scanner needs is not
      // the contrast a palette is tuned for.
      color: { dark: "#000000ff", light: "#ffffffff" },
    })
      .then(() => {
        if (!cancelled) setFailed(false);
      })
      .catch(() => {
        // The dialog still shows the code as text to read out, so a failed
        // draw degrades rather than blocking the pairing.
        if (!cancelled) setFailed(true);
      });

    return () => {
      cancelled = true;
      // Wiped on the way out. A canvas that keeps its bitmap keeps the
      // credential rendered in memory for as long as the element is alive.
      canvas.getContext("2d")?.clearRect(0, 0, canvas.width, canvas.height);
    };
  }, [value, size]);

  if (failed) {
    return (
      <p className="text-xs text-muted-foreground">
        The code couldn&apos;t be drawn. Read it out instead.
      </p>
    );
  }

  return (
    <canvas
      ref={canvasRef}
      role="img"
      aria-label={label}
      className="rounded-md bg-white p-2"
      style={{ width: size, height: size }}
    />
  );
}
