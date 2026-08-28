import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { PairingController } from "@/features/pairing/hooks/usePairing";
import { IpcFailure } from "@/lib/errors";
import type { PairingCeremony } from "@/lib/ipc";
import { PairingProgressCard } from "./PairingProgressCard";

describe("PairingProgressCard", () => {
  const awaiting: PairingCeremony = {
    ceremony_id: "ceremony-1",
    role: "initiator",
    state: "awaiting_confirmation",
    semantics: {
      message_id: "compare_codes", icon: "shieldCheck", tone: "warning", live: "status", active: true, terminal: false, needs_devices: true, review_secure: true, retry: false,
    },
    presentation: "presented",
    known_device: null,
    error: null,
  };

  it("overrides stale ceremony semantics for a safe client request failure", () => {
    const retry = vi.fn();
    const pairing = {
      ceremony: awaiting,
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
    expect(screen.queryByRole("button", { name: "Cancel" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Review securely" })).toBeNull();
  });

  it("does not offer retry for a nonretry pairing limit failure", () => {
    const pairing = {
      ceremony: awaiting,
      error: new IpcFailure("pairing_limit", false),
      isChecking: false,
      isPending: false,
      canRetry: true,
      protectedPresentationAvailable: true,
      retry: vi.fn(),
    } as unknown as PairingController;

    render(<PairingProgressCard pairing={pairing} />);
    expect(screen.getByRole("alert").textContent).toContain("as many devices");
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
  });
});
