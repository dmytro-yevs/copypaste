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
/** How long (ms) before the end of GRANTED_CONFIRMATION_MS to start fading out. */
const GRANTED_FADE_MS = 500;

export function AccessibilityBanner({
  axGranted,
  axDismissed,
  onDismiss,
  onOpenSettings,
}: AccessibilityBannerProps) {
  // CopyPaste-xn95: track whether we transitioned from not-granted → granted
  // while the banner was visible so we can show positive feedback.
  const [showGranted, setShowGranted] = useState(false);
  // CopyPaste-5917.103: track whether we are in the fade-out phase so sighted
  // users see the banner transitioning away (visual ephemerality cue).
  const [grantedFading, setGrantedFading] = useState(false);
  const grantedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const fadeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Watch for the granted transition — only fire when we were previously showing
  // the "not granted" banner (i.e. not dismissed) and axGranted flips to true.
  const prevGranted = useRef(axGranted);
  useEffect(() => {
    if (!prevGranted.current && axGranted && !axDismissed) {
      // Permission was just granted while the banner was visible — show feedback.
      setShowGranted(true);
      setGrantedFading(false);
      if (grantedTimerRef.current !== null) clearTimeout(grantedTimerRef.current);
      if (fadeTimerRef.current !== null) clearTimeout(fadeTimerRef.current);
      // Start fade-out 500ms before the banner disappears so sighted users get
      // a visual signal that the confirmation is transient (5917.103 fix).
      fadeTimerRef.current = setTimeout(() => {
        setGrantedFading(true);
      }, GRANTED_CONFIRMATION_MS - GRANTED_FADE_MS);
      grantedTimerRef.current = setTimeout(() => {
        setShowGranted(false);
        setGrantedFading(false);
      }, GRANTED_CONFIRMATION_MS);
    }
    prevGranted.current = axGranted;
  }, [axGranted, axDismissed]);

  useEffect(() => {
    return () => {
      if (grantedTimerRef.current !== null) clearTimeout(grantedTimerRef.current);
      if (fadeTimerRef.current !== null) clearTimeout(fadeTimerRef.current);
    };
  }, []);

  // Show the "granted" confirmation banner for a few seconds after permission is granted.
  if (showGranted) {
    return (
      <div
        role="status"
        aria-live="polite"
        data-testid="granted-banner"
        // 5917.103: fade-out phase flag preserved as a data hook for the redesign
        // to animate the transient-confirmation cue (styling was stripped; logic kept).
        data-fading={grantedFading}
        // Inform assistive tech that the confirmation will disappear shortly.
        aria-label="Accessibility permission granted — closing shortly"
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
      <div>
        <button type="button" onClick={onOpenSettings}>
          Open Settings
        </button>
        <button type="button" onClick={onDismiss}>
          Dismiss
        </button>
      </div>
    </div>
  );
}
