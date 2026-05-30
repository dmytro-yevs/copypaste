import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { ErrorBoundary } from "../components/ErrorBoundary";

// A stand-in for <Popup/> that throws on render, simulating the failure mode
// main.tsx guards against: a render/effect throw in the quick-paste window.
function ThrowingPopup(): never {
  throw new Error("popup boom");
}

describe("popup window error boundary", () => {
  it("shows the fallback instead of blanking when the popup subtree throws", () => {
    // The boundary logs the caught error; silence it to keep test output clean.
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    try {
      render(
        <ErrorBoundary label="Quick paste">
          <ThrowingPopup />
        </ErrorBoundary>,
      );

      // Fallback is shown with the popup's label, so the window is not blank.
      expect(
        screen.getByText(/Something went wrong in Quick paste/),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: /Retry/ }),
      ).toBeInTheDocument();
    } finally {
      spy.mockRestore();
    }
  });
});
