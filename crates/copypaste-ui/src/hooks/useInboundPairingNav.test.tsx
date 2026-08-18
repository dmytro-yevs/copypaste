import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";

import {
  inboundPairingNeedsDevices,
  useInboundPairingNav,
} from "@/hooks/usePairing";
import type { PairingCeremony } from "@/lib/ipc";
import { useUi } from "@/store/ui";
import { testClient } from "@/test/harness";

const getPairingProgress = vi.fn();
const hasBridge = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    getPairingProgress: () => getPairingProgress(),
    hasBridge: () => hasBridge(),
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

function wrapper(client = testClient()) {
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return Wrapper;
}

beforeEach(() => {
  hasBridge.mockReset().mockReturnValue(true);
  getPairingProgress.mockReset().mockResolvedValue(ceremony());
  useUi.setState({ view: "history", settingsTab: null });
});

describe("inboundPairingNeedsDevices", () => {
  it("opens Devices only when a ceremony needs a visible SAS decision", () => {
    expect(inboundPairingNeedsDevices("idle")).toBe(false);
    expect(inboundPairingNeedsDevices("waiting_for_peer")).toBe(false);
    expect(inboundPairingNeedsDevices("handshaking")).toBe(true);
    expect(inboundPairingNeedsDevices("awaiting_confirmation")).toBe(true);
    expect(inboundPairingNeedsDevices("confirmed")).toBe(false);
  });
});

describe("useInboundPairingNav", () => {
  it("navigates to Devices when confirmation arrives off that screen", async () => {
    getPairingProgress.mockResolvedValue(
      ceremony({
        ceremony_id: "inbound-1",
        role: "responder",
        state: "awaiting_confirmation",
      }),
    );
    renderHook(() => useInboundPairingNav(), { wrapper: wrapper() });

    await waitFor(() => expect(useUi.getState().view).toBe("devices"));
  });

  it("does not steal an explicit Devices session or poll while there", async () => {
    useUi.setState({ view: "devices" });
    getPairingProgress.mockResolvedValue(
      ceremony({
        ceremony_id: "inbound-2",
        role: "responder",
        state: "awaiting_confirmation",
      }),
    );
    renderHook(() => useInboundPairingNav(), { wrapper: wrapper() });

    await waitFor(() => expect(getPairingProgress).not.toHaveBeenCalled());
    expect(useUi.getState().view).toBe("devices");
  });
});
