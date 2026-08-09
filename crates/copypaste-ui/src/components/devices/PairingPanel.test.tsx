import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { PairingPanel } from "@/components/devices/PairingPanel";
import { DISCOVERED_KEY, PEERS_KEY } from "@/hooks/useDevices";
import type { PairingCeremony } from "@/lib/ipc";
import { testClient, withUser } from "@/test/harness";

const createPairingInvite = vi.fn();
const scanPairingInvite = vi.fn();
const getPairingProgress = vi.fn();
const presentPairing = vi.fn();
const confirmPairing = vi.fn();
const rejectPairing = vi.fn();
const cancelPairing = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    createPairingInvite: () => createPairingInvite(),
    scanPairingInvite: () => scanPairingInvite(),
    getPairingProgress: () => getPairingProgress(),
    presentPairing: () => presentPairing(),
    confirmPairing: () => confirmPairing(),
    rejectPairing: () => rejectPairing(),
    cancelPairing: () => cancelPairing(),
  };
});

function ceremony(over: Partial<PairingCeremony> = {}): PairingCeremony {
  return {
    ceremony_id: null,
    role: null,
    state: "idle",
    presentation: "unavailable",
    known_device: null,
    error: null,
    ...over,
  };
}

beforeEach(() => {
  const idle = ceremony();
  createPairingInvite.mockReset().mockResolvedValue(idle);
  scanPairingInvite.mockReset().mockResolvedValue(idle);
  getPairingProgress.mockReset().mockResolvedValue(idle);
  presentPairing.mockReset().mockResolvedValue(idle);
  confirmPairing.mockReset().mockResolvedValue(idle);
  rejectPairing.mockReset().mockResolvedValue(idle);
  cancelPairing.mockReset().mockResolvedValue(idle);
});

describe("native pairing boundary", () => {
  it("starts an invite from the keyboard and never renders native-only material", async () => {
    const secret = "CPPAIR2.super-secret";
    createPairingInvite.mockResolvedValue({
      ...ceremony({
        ceremony_id: "ceremony-1",
        role: "responder",
        state: "waiting_for_peer",
        presentation: "presented",
      }),
      code: secret,
      sas: "123456",
      peer_addr: "C:\\Users\\person\\copypaste.sock",
    });
    const { user } = withUser(<PairingPanel disabled={false} />);

    await screen.findByText("Ready to pair");
    await user.tab();
    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: "Show pairing code" }),
    );
    await user.keyboard("{Enter}");

    expect(await screen.findByText("Waiting for the other device")).toBeTruthy();
    expect(createPairingInvite).toHaveBeenCalledOnce();
    expect(document.body.textContent).not.toContain(secret);
    expect(document.body.textContent).not.toContain("123456");
    expect(document.body.textContent).not.toContain("copypaste.sock");
  });

  it("joins through the native scanner without accepting plaintext input", async () => {
    scanPairingInvite.mockResolvedValue(
      ceremony({
        ceremony_id: "ceremony-2",
        role: "initiator",
        state: "handshaking",
        presentation: "presented",
      }),
    );
    const { user } = withUser(<PairingPanel disabled={false} />);

    await user.click(
      await screen.findByRole("button", { name: "Scan pairing code" }),
    );

    expect(await screen.findByText("Connecting securely")).toBeTruthy();
    expect(scanPairingInvite).toHaveBeenCalledWith();
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("keeps decision controls when native confirmation is unavailable", async () => {
    const awaiting = ceremony({
      ceremony_id: "ceremony-3",
      role: "initiator",
      state: "awaiting_confirmation",
      presentation: "unavailable",
    });
    getPairingProgress.mockResolvedValue(awaiting);
    presentPairing.mockResolvedValue({ ...awaiting, presentation: "presented" });
    confirmPairing.mockResolvedValue(awaiting);
    const { user } = withUser(<PairingPanel disabled={false} />);

    await user.click(await screen.findByRole("button", { name: "Show details" }));
    await waitFor(() => expect(presentPairing).toHaveBeenCalledOnce());
    await user.click(
      screen.getByRole("button", {
        name: "Codes match — confirm pairing in the native view",
      }),
    );
    await waitFor(() => expect(confirmPairing).toHaveBeenCalledOnce());
    expect(
      screen.getByRole("button", {
        name: "Codes match — confirm pairing in the native view",
      }),
    ).toBeTruthy();
    expect(screen.queryByText(/decision was sent/i)).toBeNull();
    expect(screen.getByText(/protected pairing view didn't open/i)).toBeTruthy();
  });

  it("rejects a mismatched code and reaches the terminal state", async () => {
    getPairingProgress.mockResolvedValue(
      ceremony({
        ceremony_id: "ceremony-mismatch",
        role: "responder",
        state: "awaiting_confirmation",
      }),
    );
    rejectPairing.mockResolvedValue(
      ceremony({
        ceremony_id: "ceremony-mismatch",
        role: "responder",
        state: "rejected",
        presentation: "presented",
      }),
    );
    const { user } = withUser(<PairingPanel disabled={false} />);

    await user.click(
      await screen.findByRole("button", {
        name: "Codes don't match — reject pairing",
      }),
    );

    expect(await screen.findByText("Pairing rejected")).toBeTruthy();
    expect(rejectPairing).toHaveBeenCalledOnce();
  });

  it("cancels an active ceremony without handling a credential", async () => {
    getPairingProgress.mockResolvedValue(
      ceremony({
        ceremony_id: "ceremony-4",
        role: "responder",
        state: "waiting_for_peer",
        presentation: "presented",
      }),
    );
    cancelPairing.mockResolvedValue(
      ceremony({
        ceremony_id: "ceremony-4",
        role: "responder",
        state: "cancelled",
      }),
    );
    const { user } = withUser(<PairingPanel disabled={false} />);

    await user.click(await screen.findByRole("button", { name: "Cancel pairing" }));

    expect(await screen.findByText("Pairing cancelled")).toBeTruthy();
    expect(cancelPairing).toHaveBeenCalledWith();
  });

  it("aborts an active ceremony when the panel unmounts", async () => {
    getPairingProgress.mockResolvedValue(
      ceremony({
        ceremony_id: "ceremony-unmount",
        role: "responder",
        state: "waiting_for_peer",
        presentation: "presented",
      }),
    );
    const { unmount } = withUser(<PairingPanel disabled={false} />);

    expect(await screen.findByText("Waiting for the other device")).toBeTruthy();
    unmount();

    await waitFor(() => expect(cancelPairing).toHaveBeenCalledOnce());
    expect(cancelPairing).toHaveBeenCalledWith();
  });

  it("fences a stale mutation from a newer ceremony and its polling", async () => {
    const oldCeremony = ceremony({
      ceremony_id: "ceremony-old",
      role: "initiator",
      state: "awaiting_confirmation",
      presentation: "unavailable",
    });
    const newCeremony = ceremony({
      ceremony_id: "ceremony-new",
      role: "responder",
      state: "waiting_for_peer",
    });
    let resolveOldPresentation!: (value: PairingCeremony) => void;
    presentPairing.mockReturnValueOnce(
      new Promise<PairingCeremony>((resolve) => {
        resolveOldPresentation = resolve;
      }),
    );
    getPairingProgress
      .mockResolvedValueOnce(oldCeremony)
      .mockResolvedValueOnce(newCeremony)
      .mockResolvedValue(
        ceremony({
          ...newCeremony,
          state: "confirmed",
          known_device: { name: "New phone", last_seen_ms: 91, online: true },
        }),
      );
    const client = testClient();
    const first = withUser(<PairingPanel disabled={false} />, client);

    await first.user.click(
      await screen.findByRole("button", { name: "Show details" }),
    );
    first.unmount();
    withUser(<PairingPanel disabled={false} />, client);
    expect(await screen.findByText("Waiting for the other device")).toBeTruthy();

    resolveOldPresentation(
      ceremony({
        ...oldCeremony,
        state: "timed_out",
        presentation: "presented",
      }),
    );

    expect(
      await screen.findByText(
        "New phone is now paired and ready to sync.",
        {},
        { timeout: 2_000 },
      ),
    ).toBeTruthy();
    expect(screen.queryByText("Pairing timed out")).toBeNull();
    expect(getPairingProgress).toHaveBeenCalledTimes(3);
  });

  it("ignores a stale poll without stopping the current ceremony poll", async () => {
    const current = ceremony({
      ceremony_id: "ceremony-current",
      role: "responder",
      state: "waiting_for_peer",
    });
    let resolveStalePoll!: (value: PairingCeremony) => void;
    getPairingProgress
      .mockResolvedValueOnce(current)
      .mockReturnValueOnce(
        new Promise<PairingCeremony>((resolve) => {
          resolveStalePoll = resolve;
        }),
      )
      .mockResolvedValue(
        ceremony({
          ...current,
          state: "confirmed",
          known_device: { name: "Current phone", last_seen_ms: 92, online: true },
        }),
      );
    withUser(<PairingPanel disabled={false} />);

    expect(await screen.findByText("Waiting for the other device")).toBeTruthy();
    await waitFor(() => expect(getPairingProgress).toHaveBeenCalledTimes(2), {
      timeout: 1_500,
    });
    resolveStalePoll(
      ceremony({
        ceremony_id: "ceremony-stale",
        role: "initiator",
        state: "timed_out",
      }),
    );

    expect(
      await screen.findByText(
        "Current phone is now paired and ready to sync.",
        {},
        { timeout: 2_000 },
      ),
    ).toBeTruthy();
    expect(screen.queryByText("Pairing timed out")).toBeNull();
    expect(getPairingProgress).toHaveBeenCalledTimes(3);
  });
});

describe("terminal and failure states", () => {
  it("polls an active ceremony until it reaches a terminal state", async () => {
    getPairingProgress
      .mockResolvedValueOnce(
        ceremony({
          ceremony_id: "ceremony-poll",
          role: "responder",
          state: "waiting_for_peer",
        }),
      )
      .mockResolvedValue(
        ceremony({
          ceremony_id: "ceremony-poll",
          role: "responder",
          state: "confirmed",
          known_device: { name: "Desk PC", last_seen_ms: 84, online: false },
        }),
      );
    withUser(<PairingPanel disabled={false} />);

    expect(await screen.findByText("Waiting for the other device")).toBeTruthy();
    expect(
      await screen.findByText(
        "Desk PC is now paired and ready to sync.",
        {},
        { timeout: 2_000 },
      ),
    ).toBeTruthy();
    expect(getPairingProgress).toHaveBeenCalledTimes(2);
  });

  it("announces timeout and retries the same native flow", async () => {
    getPairingProgress.mockResolvedValue(
      ceremony({
        ceremony_id: "ceremony-5",
        role: "responder",
        state: "timed_out",
      }),
    );
    createPairingInvite.mockResolvedValue(
      ceremony({
        ceremony_id: "ceremony-6",
        role: "responder",
        state: "waiting_for_peer",
        presentation: "presented",
      }),
    );
    const { user } = withUser(<PairingPanel disabled={false} />);

    const timeout = await screen.findByText("Pairing timed out");
    expect(timeout.closest("[role='status']")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Try again" }));

    expect(await screen.findByText("Waiting for the other device")).toBeTruthy();
    expect(createPairingInvite).toHaveBeenCalledOnce();
  });

  it("maps command failures to authored copy and supports retry", async () => {
    createPairingInvite
      .mockRejectedValueOnce({
        code: "internal",
        retryable: true,
        message: "C:\\Users\\person\\service.sock",
      })
      .mockResolvedValueOnce(
        ceremony({
          ceremony_id: "ceremony-7",
          role: "responder",
          state: "waiting_for_peer",
          presentation: "presented",
        }),
      );
    const { user } = withUser(<PairingPanel disabled={false} />);

    await user.click(
      await screen.findByRole("button", { name: "Show pairing code" }),
    );
    expect(await screen.findByText("The clipboard service returned an error.")).toBeTruthy();
    expect(document.body.textContent).not.toContain("service.sock");

    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Waiting for the other device")).toBeTruthy();
    expect(createPairingInvite).toHaveBeenCalledTimes(2);
  });

  it("shows a sanitized terminal mismatch error", async () => {
    getPairingProgress.mockResolvedValue(
      ceremony({
        ceremony_id: "ceremony-8",
        role: "initiator",
        state: "failed",
        error: { code: "peer_version", retryable: false },
      }),
    );
    withUser(<PairingPanel disabled={false} />);

    expect(
      await screen.findByText(/versions of CopyPaste that can't sync/i),
    ).toBeTruthy();
    expect(screen.getByRole("alert")).toBeTruthy();
  });

  it("refreshes both known-device lists after confirmation", async () => {
    getPairingProgress.mockResolvedValue(
      ceremony({
        ceremony_id: "ceremony-9",
        role: "initiator",
        state: "confirmed",
        presentation: "presented",
        known_device: { name: "Kitchen Mac", last_seen_ms: 42, online: true },
      }),
    );
    const client = testClient();
    const invalidate = vi.spyOn(client, "invalidateQueries");
    withUser(<PairingPanel disabled={false} />, client);

    expect(await screen.findByText("Kitchen Mac is now paired and ready to sync.")).toBeTruthy();
    await waitFor(() => {
      expect(invalidate).toHaveBeenCalledWith({ queryKey: PEERS_KEY });
      expect(invalidate).toHaveBeenCalledWith({ queryKey: DISCOVERED_KEY });
    });
  });
});
