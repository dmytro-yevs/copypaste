import { Component, type ErrorInfo, type ReactNode } from "react";
import { AlertTriangle } from "lucide-react";
import { EmptyState } from "./EmptyState";

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
      // Error detail is logged by componentDidCatch — not rendered here to
      // avoid leaking filesystem paths or internal strings into the DOM.
      <EmptyState
        icon={<AlertTriangle aria-hidden="true" />}
        title={`Something went wrong${where}`}
        body="The background service may be unavailable, or this screen failed to load. The rest of the app is still usable."
        action={
          <button type="button" className="btn btn--secondary" onClick={this.handleRetry}>
            Retry
          </button>
        }
      />
    );
  }
}
