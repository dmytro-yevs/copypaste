/**
 * CopyPaste-a7kt — per-view ErrorBoundary with localized fallback.
 *
 * Verifies that:
 *  1. A view-level crash (thrown error during render) is caught by ErrorBoundary.
 *  2. Only the view's area shows a fallback — the rest of the app stays usable.
 *  3. The fallback includes the view label for localization ("in Logs", etc.).
 *  4. A Retry button resets the boundary so the view can re-mount.
 *  5. The raw error is NOT rendered in the DOM (only logged via componentDidCatch).
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ErrorBoundary } from "./ErrorBoundary";

// Suppress React's own console.error logging about uncaught render errors
// to avoid noise in the test output. componentDidCatch is the boundary handler.
function suppressReactErrorLogs() {
  return vi.spyOn(console, "error").mockImplementation(() => {});
}

// A component that throws synchronously during render.
function Boom({ message }: { message: string }): never {
  throw new Error(message);
}

// A component that renders normally.
function Fine() {
  return <div data-testid="fine-content">All good</div>;
}

describe("ErrorBoundary — per-view localized fallback (CopyPaste-a7kt)", () => {
  it("catches a render error and shows a localized fallback", () => {
    const spy = suppressReactErrorLogs();

    render(
      <ErrorBoundary label="Logs">
        <Boom message="fs error: /Users/bob/Library/Logs" />
      </ErrorBoundary>
    );

    // Fallback must be present (no blank window).
    expect(screen.getByText(/something went wrong/i)).toBeInTheDocument();

    // Fallback MUST include the view label.
    expect(screen.getByText(/something went wrong in Logs/i)).toBeInTheDocument();

    // The raw error message must NOT appear in the DOM.
    expect(document.body.textContent).not.toContain("/Users/bob");

    spy.mockRestore();
  });

  it("shows a Retry button that resets the boundary", () => {
    const spy = suppressReactErrorLogs();

    let shouldThrow = true;

    function Conditional() {
      if (shouldThrow) throw new Error("boom");
      return <div data-testid="recovered">Recovered!</div>;
    }

    const { rerender } = render(
      <ErrorBoundary label="Devices">
        <Conditional />
      </ErrorBoundary>
    );

    // Fallback shown.
    expect(screen.getByRole("button", { name: /retry/i })).toBeInTheDocument();

    // Fix the underlying cause, then click Retry.
    shouldThrow = false;
    fireEvent.click(screen.getByRole("button", { name: /retry/i }));

    // After reset the child should render again.
    rerender(
      <ErrorBoundary label="Devices">
        <Conditional />
      </ErrorBoundary>
    );

    expect(screen.getByTestId("recovered")).toBeInTheDocument();

    spy.mockRestore();
  });

  it("does not affect sibling components outside the boundary", () => {
    const spy = suppressReactErrorLogs();

    render(
      <div>
        {/* This sibling renders OUTSIDE the boundary — it must stay usable. */}
        <Fine />
        <ErrorBoundary label="History">
          <Boom message="internal error" />
        </ErrorBoundary>
      </div>
    );

    // The crashing view shows its fallback.
    expect(screen.getByText(/something went wrong in History/i)).toBeInTheDocument();

    // The sibling is still rendered.
    expect(screen.getByTestId("fine-content")).toBeInTheDocument();

    spy.mockRestore();
  });

  it("does not render the raw error text in the fallback", () => {
    const spy = suppressReactErrorLogs();

    const sensitiveMsg = "ENOENT /Users/alice/Library/Logs/CopyPaste/daemon.log";

    render(
      <ErrorBoundary label="Logs">
        <Boom message={sensitiveMsg} />
      </ErrorBoundary>
    );

    // The fallback must not leak the raw error string.
    expect(document.body.textContent).not.toContain(sensitiveMsg);
    expect(document.body.textContent).not.toContain("/Users/alice");

    spy.mockRestore();
  });

  it("Retry button uses skin radius token (CopyPaste-5917.94): no rounded-ide class, inline borderRadius = var(--r-ctl)", () => {
    // CopyPaste-5917.94: The Retry button must use the skin radius CSS variable
    // so its corner radius adapts across classic (9px), quiet (7px), and vapor
    // (12px) skins — NOT the static rounded-ide Tailwind class.
    const spy = suppressReactErrorLogs();

    render(
      <ErrorBoundary label="Test">
        <Boom message="trigger boundary" />
      </ErrorBoundary>
    );

    const retryBtn = screen.getByRole("button", { name: /retry/i });

    // Must NOT carry the static hardcoded class.
    expect(retryBtn.classList.contains("rounded-ide")).toBe(false);

    // Must reference the skin token so the browser resolves the active skin value.
    expect(retryBtn.style.borderRadius).toBe("var(--r-ctl)");

    spy.mockRestore();
  });

  it("Retry button uses skin radius token (CopyPaste-5917.94): no rounded-ide class, inline borderRadius = var(--r-ctl)", () => {
    // CopyPaste-5917.94: The Retry button must use the skin radius CSS variable
    // so its corner radius adapts across classic (9px), quiet (7px), and vapor
    // (12px) skins — NOT the static rounded-ide Tailwind class.
    const spy = suppressReactErrorLogs();

    render(
      <ErrorBoundary label="Test">
        <Boom message="trigger boundary" />
      </ErrorBoundary>
    );

    const retryBtn = screen.getByRole("button", { name: /retry/i });

    // Must NOT carry the static hardcoded class.
    expect(retryBtn.classList.contains("rounded-ide")).toBe(false);

    // Must reference the skin token so the browser resolves the active skin value.
    expect(retryBtn.style.borderRadius).toBe("var(--r-ctl)");

    spy.mockRestore();
  });
});
