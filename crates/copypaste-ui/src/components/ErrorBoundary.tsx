import { Component, type ErrorInfo, type ReactNode } from "react";

interface ErrorBoundaryProps {
  /** Where this boundary sits, used in the fallback heading (e.g. "Settings"). */
  label?: string;
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

/**
 * Library-free React error boundary.
 *
 * A thrown error during render or in an effect (e.g. an unhandled IPC rejection
 * surfaced through render, or an unstable store selector) would otherwise
 * unmount the entire React tree, leaving a blank window. This catches the
 * throw, shows a readable fallback with a Retry button, and keeps the rest of
 * the app (tray, other views) usable. Retry resets the boundary so re-rendering
 * the subtree is attempted again — useful once the daemon comes back.
 */
export class ErrorBoundary extends Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // Surface to the console for diagnostics; never re-throw (that would blank
    // the window again).
    // eslint-disable-next-line no-console
    console.error("[ErrorBoundary] caught render/effect error:", error, info);
  }

  private handleRetry = () => {
    this.setState({ error: null });
  };

  render() {
    const { error } = this.state;
    if (error === null) return this.props.children;

    const where = this.props.label ? ` in ${this.props.label}` : "";
    return (
      <div className="flex h-full min-h-0 flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
        <div className="text-[14px] font-medium text-ide-text">
          Something went wrong{where}
        </div>
        <div className="max-w-sm text-[12px] text-ide-dim">
          The background service may be unavailable, or this screen failed to
          load. The rest of the app is still usable.
        </div>
        {/* Error detail is logged by componentDidCatch — not rendered here
            to avoid leaking filesystem paths or internal strings into the DOM. */}
        <button
          type="button"
          onClick={this.handleRetry}
          className={[
            "mt-1 rounded-ide border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-text",
            "hover:bg-ide-hover",
          ].join(" ")}
        >
          Retry
        </button>
      </div>
    );
  }
}
