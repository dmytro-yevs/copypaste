import { AlertTriangle, CheckCircle2, Info, ServerCrash } from "lucide-react";

// Banners — one component, four severities (patterns.css `.banner--warn`/
// `--err`/`--info`/`--ok`, task 6.7). Markup mirrors the real call sites
// (App.tsx's daemon-error/protocol-mismatch banners, AccessibilityBanner).
export function BannersSection() {
  return (
    <section id="gallery-banners">
      <h2>Banners — all four severities</h2>
      <div className="gallery__col">
        <div className="banner banner--err" role="alert">
          <ServerCrash aria-hidden="true" />
          <span className="banner__x">
            <b>Background service error:</b> The background service failed to
            start. Please reinstall CopyPaste or restart your Mac.
          </span>
        </div>
        <div className="banner banner--warn" role="alert">
          <AlertTriangle aria-hidden="true" />
          <span className="banner__x">
            CopyPaste app and background service are on incompatible versions.
            Restart the app or the background service to resolve.
          </span>
          <span className="banner__act">
            <button type="button" className="btn">
              Dismiss
            </button>
          </span>
        </div>
        <div className="banner banner--info" role="status">
          <Info aria-hidden="true" />
          <span className="banner__x">
            Cloud sync is using a relay fallback — expect brief delays.
          </span>
        </div>
        <div className="banner banner--ok" role="status" aria-live="polite">
          <CheckCircle2 aria-hidden="true" />
          <span className="banner__x">
            Accessibility permission granted — global paste shortcut and
            hotkey capture are active.
          </span>
        </div>
      </div>
    </section>
  );
}
