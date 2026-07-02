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

  // Auto-dismiss timer
  useEffect(() => {
    const t = setTimeout(() => onDismiss(msg.id), duration);
    return () => clearTimeout(t);
  }, [msg.id, duration, onDismiss]);

  return (
    // .toast/.show: patterns.css toast pill (design.md Decision 13/X5).
    // aria-live="polite": announced without interrupting the user.
    <div
      role="status"
      aria-live="polite"
      className="toast show"
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
// ToastContainer — portal that renders all active toasts at bottom-center
// ---------------------------------------------------------------------------

function ToastContainer({ toasts, onDismiss }: { toasts: ToastMessage[]; onDismiss: (id: string) => void }) {
  if (toasts.length === 0) return null;
  return ReactDOM.createPortal(
    <div
      // Stack at bottom-center, same as iOS toast convention. z-40 keeps it
      // below modals (z-50) but above regular content. Mirrors the undo-toast
      // in HistoryView (SCRH-12) — transient notifications must not occlude dialogs.
      aria-live="polite"
      aria-atomic="false"
    >
      {toasts.map((msg) => (
        <div key={msg.id}>
          <GlassToastItem msg={msg} onDismiss={onDismiss} />
        </div>
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
