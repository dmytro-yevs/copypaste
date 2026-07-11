import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";
import ReactDOM from "react-dom";
import { X } from "lucide-react";

// ---------------------------------------------------------------------------
// GlassToast — web equivalent of Android GlassToast (CopyPaste-1a4t)
//
// Semantics mirror Android GlassToast.kt:
//   - aria-live="polite" (= LiveRegionMode.Polite)
//   - surface-card glass class (floating frosted layer)
//   - Auto-dismiss after `duration` ms (default 3 000)
//   - Animated via the CSS `.toast-enter` class (approved entrance motion §MO-6)
//   - Stacked: newer toasts appear above older ones
// ---------------------------------------------------------------------------

export type ToastKind = "info" | "success" | "error" | "warning";

export interface ToastMessage {
  id: string;
  text: string;
  kind?: ToastKind;
  duration?: number;
}

/**
 * Map a ToastKind to the leading `.dot-stat` colour (patterns.css `.toast`,
 * design.md Decision 13/X5). `.dot-stat` itself only ships on/off (ok/err)
 * variants, so the full four-severity palette is applied inline here — same
 * approach as SyncStatusChip's status dot.
 */
function toastDotColor(kind: ToastKind | undefined): string {
  switch (kind) {
    case "success":
      return "var(--ok)";
    case "warning":
      return "var(--warn)";
    case "error":
      return "var(--err)";
    case "info":
    default:
      return "var(--info)";
  }
}

// ---------------------------------------------------------------------------
// Internal GlassToastItem — one rendered toast bubble
// ---------------------------------------------------------------------------

function GlassToastItem({
  msg,
  onDismiss,
}: {
  msg: ToastMessage;
  onDismiss: (id: string) => void;
}) {
  const duration = msg.duration ?? 3000;

  // CopyPaste-8ebg.55: pause the auto-dismiss timer while the user is
  // hovering or has focus inside the toast (e.g. tabbed to the Dismiss
  // button) — otherwise a toast can vanish mid-read/mid-interaction.
  // `paused` only gates whether a NEW timer is armed; the remaining time is
  // not tracked precisely (a toast is a low-stakes transient notice), so on
  // resume it simply restarts a full-duration timer.
  const [paused, setPaused] = useState(false);

  useEffect(() => {
    if (paused) return;
    const t = setTimeout(() => onDismiss(msg.id), duration);
    return () => clearTimeout(t);
  }, [msg.id, duration, onDismiss, paused]);

  return (
    // .toast/.show: patterns.css toast pill (design.md Decision 13/X5).
    // aria-live="polite": announced without interrupting the user.
    <div
      role="status"
      aria-live="polite"
      className="toast show"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocus={() => setPaused(true)}
      onBlur={() => setPaused(false)}
    >
      {/* VISM-11: leading semantic colour dot — visual consistency with HistoryView toasts */}
      <span
        className="dot-stat"
        style={{ background: toastDotColor(msg.kind) }}
        aria-hidden="true"
      />
      <span>{msg.text}</span>
      <button
        type="button"
        className="iconbtn"
        aria-label="Dismiss"
        onClick={() => onDismiss(msg.id)}
      >
        <X aria-hidden="true" />
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// ToastContainer — portal that renders all active toasts at bottom-right
// ---------------------------------------------------------------------------

function ToastContainer({ toasts, onDismiss }: { toasts: ToastMessage[]; onDismiss: (id: string) => void }) {
  if (toasts.length === 0) return null;
  return ReactDOM.createPortal(
    <div
      // CopyPaste-7w060.2: stack at bottom-right, not bottom-center — the shell's
      // persistent left sidebar (and its footer SyncStatusChip) occupies the same
      // bottom band, so centering on the full viewport bled the stack into it at
      // narrow window widths. Mirrors the undo-toast in HistoryView (SCRH-12) —
      // transient notifications must not occlude dialogs.
      //
      // CopyPaste-8ebg.38: `.toast-stack` positions/stacks the whole group (patterns.css);
      // individual `.toast` items are laid out in normal flow inside it via
      // flex-direction: column-reverse (newest message added last in the array, so
      // column-reverse puts it visually first/closest to the screen edge) instead of
      // every toast independently self-positioning to the same fixed spot and
      // rendering exactly on top of each other.
      className="toast-stack"
      aria-live="polite"
      aria-atomic="false"
    >
      {toasts.map((msg) => (
        <GlassToastItem key={msg.id} msg={msg} onDismiss={onDismiss} />
      ))}
    </div>,
    document.body,
  );
}

// ---------------------------------------------------------------------------
// ToastContext + ToastProvider
// ---------------------------------------------------------------------------

interface ToastContextValue {
  show: (text: string, options?: { kind?: ToastKind; duration?: number }) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

let _nextId = 0;

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  const dismiss = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const show = useCallback(
    (text: string, options?: { kind?: ToastKind; duration?: number }) => {
      const id = `toast-${++_nextId}`;
      setToasts((prev) => [...prev, { id, text, kind: options?.kind, duration: options?.duration }]);
    },
    [],
  );

  return (
    <ToastContext.Provider value={{ show }}>
      {children}
      <ToastContainer toasts={toasts} onDismiss={dismiss} />
    </ToastContext.Provider>
  );
}

// ---------------------------------------------------------------------------
// useToast — hook for consuming components
// ---------------------------------------------------------------------------

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext);
  // Graceful degradation: if no provider is mounted, return a no-op so callers
  // don't crash when used outside a ToastProvider during tests.
  if (!ctx) {
    return {
      show: () => {
        // no-op outside provider
      },
    };
  }
  return ctx;
}

// ---------------------------------------------------------------------------
// Standalone GlassToast export — for direct use without provider
// ---------------------------------------------------------------------------

export { GlassToastItem as GlassToast };
