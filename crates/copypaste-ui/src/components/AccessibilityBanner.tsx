/**
 * AccessibilityBanner — macOS-only Accessibility permission prompt.
 *
 * CopyPaste-xn95: extracted from App.tsx so the component can be tested in
 * isolation. The banner shows a warning when `axGranted` is false and
 * positive feedback ("Accessibility permission granted") when it transitions
 * from false → true, so the user knows their action succeeded rather than
 * the UI just silently updating.
 *
 * Props are controlled by App.tsx which polls `checkAccessibilityPermission()`
 * every 3 s while the banner is visible.
 */
import { useEffect, useRef, useState } from "react";

interface AccessibilityBannerProps {
  /** True once checkAccessibilityPermission() returned true. */
  axGranted: boolean;
  /** True once the user has dismissed the banner. */
  axDismissed: boolean;
  onDismiss: () => void;
  onOpenSettings: () => void;
}

/** How long (ms) to show the "granted" confirmation before it auto-hides. */
const GRANTED_CONFIRMATION_MS = 3000;

export function AccessibilityBanner({
  axGranted,
  axDismissed,
  onDismiss,
  onOpenSettings,
}: AccessibilityBannerProps) {
  // CopyPaste-xn95: track whether we transitioned from not-granted → granted
  // while the banner was visible so we can show positive feedback.
  const [showGranted, setShowGranted] = useState(false);
  const grantedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Watch for the granted transition — only fire when we were previously showing
  // the "not granted" banner (i.e. not dismissed) and axGranted flips to true.
  const prevGranted = useRef(axGranted);
  useEffect(() => {
    if (!prevGranted.current && axGranted && !axDismissed) {
      // Permission was just granted while the banner was visible — show feedback.
      setShowGranted(true);
      if (grantedTimerRef.current !== null) clearTimeout(grantedTimerRef.current);
      grantedTimerRef.current = setTimeout(() => {
        setShowGranted(false);
      }, GRANTED_CONFIRMATION_MS);
    }
    prevGranted.current = axGranted;
  }, [axGranted, axDismissed]);

  useEffect(() => {
    return () => {
      if (grantedTimerRef.current !== null) clearTimeout(grantedTimerRef.current);
    };
  }, []);

  // Show the "granted" confirmation banner for a few seconds after permission is granted.
  if (showGranted) {
    return (
      <div
        className="surface-glass flex shrink-0 items-center gap-3 border border-ide-success/40 px-3 py-2 text-[13px] text-ide-success"
        style={{ borderRadius: "var(--skin-r-card)" }}
        role="status"
        aria-live="polite"
      >
        {/* xn95: positive confirmation so the user sees their action succeeded. */}
        Accessibility permission granted — global paste shortcut and hotkey capture are active.
      </div>
    );
  }

  // Hide when already granted or dismissed.
  if (axGranted || axDismissed) return null;

  return (
    <div
      className="surface-glass flex shrink-0 items-start justify-between gap-3 border border-ide-warning/40 px-3 py-2 text-[13px] text-ide-warning"
      style={{ borderRadius: "var(--skin-r-card)" }}
      // A11Y-2 / CopyPaste-5917.3: assertive live region so screen readers announce
      // the permission warning immediately when it appears, without waiting for
      // the user to navigate to it. "polite" is already used for the granted
      // confirmation above; warning state is urgent enough to interrupt.
      role="alert"
      aria-live="assertive"
    >
      <span>
        Accessibility permission is required for the global paste shortcut
        and hotkey capture. Grant it in System Settings to enable these
        features.
      </span>
      <div className="flex shrink-0 items-center gap-2">
        <button
          type="button"
          onClick={onOpenSettings}
          className="border border-ide-warning/50 bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-warning hover:bg-ide-hover"
          style={{ borderRadius: "var(--skin-r-ctl)" }}
        >
          Open Settings
        </button>
        <button
          type="button"
          onClick={onDismiss}
          className="border border-ide-border bg-ide-panel px-2.5 py-1 text-[12px] text-ide-text hover:bg-ide-hover"
          style={{ borderRadius: "var(--skin-r-ctl)" }}
        >
          Dismiss
        </button>
      </div>
    </div>
  );
}
