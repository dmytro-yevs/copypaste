import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import type { SecurePairingView, SecureInviteView } from "@/generated/ipc";
import ProtectedPairingSurface from "./ProtectedPairingSurface";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

function view(phase: "invite" | "join"): SecurePairingView {
  return {
    generation: 7,
    phase,
    ceremony: {
      ceremony_id: phase === "invite" ? "ceremony-one" : null,
      role: phase === "invite" ? "initiator" : null,
      state: phase === "invite" ? "waiting_for_peer" : "idle",
      semantics: {} as SecurePairingView["ceremony"]["semantics"],
      presentation: "presented",
      known_device: null,
      error: null,
    },
  };
}

function mount() {
  return render(<TooltipProvider><ProtectedPairingSurface /></TooltipProvider>);
}

describe("protected pairing window", () => {
  beforeEach(() => { invokeMock.mockReset(); });

  it("drops a delayed invite response after the window is hidden", async () => {
    let deliver!: (value: SecureInviteView) => void;
    const delayed = new Promise<SecureInviteView>((resolve) => { deliver = resolve; });
    invokeMock.mockImplementation((command) => {
      if (command === "pair_secure_state") return Promise.resolve(view("invite"));
      if (command === "pair_secure_reveal_invite") return delayed;
      return Promise.resolve();
    });
    mount();
    fireEvent.click(await screen.findByRole("button", { name: "Reveal code" }));
    expect(invokeMock).toHaveBeenCalledWith("pair_secure_reveal_invite", { generation: 7 });
    act(() => { window.dispatchEvent(new Event("pagehide")); });
    await act(async () => {
      deliver({ generation: 7, ceremony_id: "ceremony-one", code: "secret-code", address: "192.0.2.1:4", qr_svg: "<svg></svg>", expires_in_ms: 60_000 });
      await delayed;
    });
    await waitFor(() => expect(screen.queryByText("secret-code")).toBeNull());
    expect(screen.queryByAltText("Pairing QR code")).toBeNull();
  });

  it("removes manual join fields from the DOM on page hide", async () => {
    invokeMock.mockImplementation((command) => command === "pair_secure_state"
      ? Promise.resolve(view("join"))
      : Promise.resolve());
    mount();
    const code = await screen.findByLabelText("Pairing code") as HTMLInputElement;
    const address = screen.getByLabelText("Pairing address") as HTMLInputElement;
    fireEvent.change(code, { target: { value: "secret-code" } });
    fireEvent.change(address, { target: { value: "192.0.2.1:4" } });
    act(() => { window.dispatchEvent(new Event("pagehide")); });
    expect(code.value).toBe("");
    expect(address.value).toBe("");
  });
});
