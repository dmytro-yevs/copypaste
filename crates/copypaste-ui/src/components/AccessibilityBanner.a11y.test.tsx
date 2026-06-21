/**
 * A11Y-2 / CopyPaste-5917.3: AccessibilityBanner warning state must have
 * an ARIA live region so screen readers announce it immediately.
 *
 * - Warning banner (axGranted=false): role="alert" + aria-live="assertive"
 * - Granted confirmation (showGranted=true): role="status" + aria-live="polite"
 */

import { describe, it, expect, vi } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { AccessibilityBanner } from "./AccessibilityBanner";

describe("A11Y-2 / CopyPaste-5917.3: AccessibilityBanner ARIA live regions", () => {
  it("renders warning banner with role=alert and aria-live=assertive when permission is not granted", () => {
    render(
      <AccessibilityBanner
        axGranted={false}
        axDismissed={false}
        onDismiss={vi.fn()}
        onOpenSettings={vi.fn()}
      />
    );

    const alert = screen.getByRole("alert");
    expect(alert).toBeInTheDocument();
    expect(alert).toHaveAttribute("aria-live", "assertive");
  });

  it("renders granted confirmation with role=status and aria-live=polite on grant transition", async () => {
    const { rerender } = render(
      <AccessibilityBanner
        axGranted={false}
        axDismissed={false}
        onDismiss={vi.fn()}
        onOpenSettings={vi.fn()}
      />
    );

    // Simulate permission being granted while the banner is visible.
    await act(async () => {
      rerender(
        <AccessibilityBanner
          axGranted={true}
          axDismissed={false}
          onDismiss={vi.fn()}
          onOpenSettings={vi.fn()}
        />
      );
    });

    // The granted confirmation uses role="status" + aria-live="polite" (already in code).
    const status = screen.getByRole("status");
    expect(status).toBeInTheDocument();
    expect(status).toHaveAttribute("aria-live", "polite");
  });

  it("renders nothing when already granted and not in transition", () => {
    const { container } = render(
      <AccessibilityBanner
        axGranted={true}
        axDismissed={false}
        onDismiss={vi.fn()}
        onOpenSettings={vi.fn()}
      />
    );

    expect(container.firstChild).toBeNull();
  });

  it("renders nothing when dismissed", () => {
    const { container } = render(
      <AccessibilityBanner
        axGranted={false}
        axDismissed={true}
        onDismiss={vi.fn()}
        onOpenSettings={vi.fn()}
      />
    );

    expect(container.firstChild).toBeNull();
  });

  it("warning banner contains the permission prompt text", () => {
    render(
      <AccessibilityBanner
        axGranted={false}
        axDismissed={false}
        onDismiss={vi.fn()}
        onOpenSettings={vi.fn()}
      />
    );

    expect(screen.getByRole("alert").textContent).toMatch(/Accessibility permission/i);
  });
});
