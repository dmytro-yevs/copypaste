import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { PairingController } from "@/features/pairing/hooks/usePairing";
import { IpcFailure } from "@/lib/errors";
import { PairingProgressCard } from "./PairingProgressCard";

describe("PairingProgressCard", () => {
  it("overrides stale ceremony semantics for a safe client request failure", () => {
    const retry = vi.fn();
    const pairing = {
      ceremony: undefined,
      error: new IpcFailure("peer_unreachable", true),
      isChecking: false,
      isPending: false,
      canRetry: true,
      protectedPresentationAvailable: true,
      retry,
    } as unknown as PairingController;

    const { container } = render(<PairingProgressCard pairing={pairing} />);
    const alert = screen.getByRole("alert");
    expect(alert.getAttribute("aria-live")).toBe("assertive");
    expect(alert.textContent).not.toContain("peer_unreachable");
    expect(container.querySelector('[data-state="client_error"]')).toBeTruthy();
    expect(container.querySelector('[data-tone="danger"]')).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });
});
