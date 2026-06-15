import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";
import ReactDOM from "react-dom";

// ---------------------------------------------------------------------------
// GlassToast — web equivalent of Android GlassToast (CopyPaste-1a4t)
//
// Semantics mirror Android GlassToast.kt:
//   - aria-live="polite" (= LiveRegionMode.Polite)
//   - surface-card glass class (floating frosted layer)
//   - Auto-dismiss after `duration` ms (default 3 000)
//   - Animated via CSS .toast-in (already defined in tailwind keyframes)
//   - Stacked: newer toasts appear above older ones
// ---------------------------------------------------------------------------

export type ToastKind = "info" | "success" | "error" | "warning";

export interface ToastMessage {
  id: string;
  text: string;
  kind?: ToastKind;
  duration?: number;
}

// ---------------------------------------------------------------------------
// Internal GlassToastItem — one rendered toast bubble
// ---------------------------------------------------------------------------

const KIND_CLS: Record<ToastKind, string> = {
  info:    "text-ide-text",
  success: "text-ide-success",
  error:   "text-ide-danger",
  warning: "text-ide-warning",
};

function GlassToastItem({
  msg,
  onDismiss,
}: {
  msg: ToastMessage;
  onDismiss: (id: string) => void;
}) {
  const duration = msg.duration ?? 3000;
  const kind = msg.kind ?? "info";

  // Auto-dismiss timer
  useEffect(() => {
    const t = setTimeout(() => onDismiss(msg.id), duration);
    return () => clearTimeout(t);
  }, [msg.id, duration, onDismiss]);

  return (
    // surface-card: glass float over aurora canvas (spec §surface-card)
    // animate-toast-in: slide-up entrance (tailwind keyframes §8)
    <div
      role="status"
      aria-live="polite"
      className={[
        "surface-card animate-toast-in",
        "min-w-[200px] max-w-[340px] rounded-ide-lg px-4 py-2.5 shadow-ide-sm",
        "flex items-center justify-between gap-3",
        KIND_CLS[kind],
      ].join(" ")}
    >
      <span className="text-[13px] leading-snug">{msg.text}</span>
      <button
        type="button"
        aria-label="Dismiss"
        onClick={() => onDismiss(msg.id)}
        className="shrink-0 text-ide-faint hover:text-ide-dim transition-colors"
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor" aria-hidden="true">
          <path d="M1 1l10 10M11 1L1 11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" fill="none" />
        </svg>
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
      // Stack at bottom-center, same as iOS toast convention. z-[9998] keeps it
      // below modals (z-50) but above all other content.
      className="fixed bottom-6 left-1/2 -translate-x-1/2 z-[9998] flex flex-col gap-2 items-center pointer-events-none"
      aria-live="polite"
      aria-atomic="false"
    >
      {toasts.map((msg) => (
        <div key={msg.id} className="pointer-events-auto">
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
