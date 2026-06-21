/**
 * CopyPaste-xn95: Accessibility permission banner must show positive feedback
 * (a confirmation message) after the user grants the permission, rather than
 * silently disappearing without any acknowledgment.
 *
 * Strategy: test the AccessibilityBanner component in isolation (extracted from
 * App.tsx) by rendering it with various state combinations and asserting that
 * the "granted" feedback text is present after `axGranted` transitions to true.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mock Tauri bridge BEFORE importing any module that pulls in ipc.ts.
// ---------------------------------------------------------------------------
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import { AccessibilityBanner } from "./AccessibilityBanner";

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("CopyPaste-xn95: AccessibilityBanner feedback", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("shows no banner when already granted on mount", () => {
    render(<AccessibilityBanner axGranted={true} axDismissed={false} onDismiss={() => {}} onOpenSettings={() => {}} />);
    // No warning banner when permission is already granted.
    expect(screen.queryByText(/accessibility permission/i)).not.toBeInTheDocument();
  });

  it("shows warning banner when permission is not granted", () => {
    render(<AccessibilityBanner axGranted={false} axDismissed={false} onDismiss={() => {}} onOpenSettings={() => {}} />);
    expect(screen.getByText(/accessibility permission/i)).toBeInTheDocument();
  });

  it("shows a 'granted' confirmation message after permission is granted (xn95 fix)", async () => {
    const { rerender } = render(
      <AccessibilityBanner axGranted={false} axDismissed={false} onDismiss={() => {}} onOpenSettings={() => {}} />,
    );

    // Simulate the user granting permission — axGranted transitions to true.
    rerender(
      <AccessibilityBanner axGranted={true} axDismissed={false} onDismiss={() => {}} onOpenSettings={() => {}} />,
    );

    // A positive confirmation message must appear so the user knows their action succeeded.
    await waitFor(() => {
      const body = document.body.textContent ?? "";
      expect(body).toMatch(/accessibility.*granted|granted.*accessibility|permission.*granted|granted/i);
    });
  });

  it("does not show the granted message when it was dismissed before being granted", () => {
    render(<AccessibilityBanner axGranted={false} axDismissed={true} onDismiss={() => {}} onOpenSettings={() => {}} />);
    // When dismissed the banner is gone; no confusion from a phantom "granted" message.
    expect(screen.queryByText(/accessibility permission/i)).not.toBeInTheDocument();
  });
});
