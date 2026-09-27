import { createElement, type ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { PairingCeremony } from "@/lib/ipc";
import { useUi } from "@/store/ui";

const native = vi.hoisted(() => ({
  listener: undefined as ((event: { payload: { topic: string } }) => void) | undefined,
  progress: vi.fn(),
}));

vi.mock("@/lib/tauriEventRegistry", () => ({
  subscribeNativeEvent: (_event: string, listener: typeof native.listener) => {
    native.listener = listener;
    return () => { native.listener = undefined; };
  },
}));

vi.mock("@/lib/ipc", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/ipc")>()),
  hasBridge: () => true,
  getPairingProgress: () => native.progress(),
}));

import {
  useInboundPairingNav,
} from "./useInboundPairingNav";
import { pairingProgressInterval } from "./usePairingProgress";
import { PAIRING_POLL_MS } from "@/features/pairing/model/pairingSession";

const IDLE = {
  ceremony_id: null,
  role: null,
  state: "idle",
  semantics: {
    message_id: "ready",
    icon: "shieldCheck",
    tone: "neutral",
    live: "status",
    active: false,
    terminal: false,
    needs_devices: false,
    review_secure: false,
    retry: false,
    copy: { title: "Pair a device", detail: "No device pairing is in progress." },
  },
  presentation: "available",
  known_device: null,
  error: null,
} as const satisfies PairingCeremony;

const INBOUND = {
  ...IDLE,
  ceremony_id: "inbound-1",
  role: "responder",
  state: "waiting_for_peer",
  semantics: { ...IDLE.semantics, active: true, needs_devices: true },
} as const satisfies PairingCeremony;

function wrapper(client: QueryClient) {
  return ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client }, children);
}

function renderInboundNav() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return renderHook(() => useInboundPairingNav(), { wrapper: wrapper(client) });
}

describe("useInboundPairingNav", () => {
  beforeEach(() => {
    native.listener = undefined;
    native.progress.mockReset().mockResolvedValue(IDLE);
    useUi.setState({ view: "history" });
  });

  it("opens Devices when the shared peer-change event reports an inbound pairing", async () => {
    renderInboundNav();
    await waitFor(() => expect(native.progress).toHaveBeenCalledOnce());

    native.progress.mockResolvedValueOnce(INBOUND);
    await act(async () => {
      native.listener?.({ payload: { topic: "peers" } });
    });

    await waitFor(() => expect(useUi.getState().view).toBe("devices"));
  });

  it("rechecks on resume so a missed native event still opens Devices", async () => {
    renderInboundNav();
    await waitFor(() => expect(native.progress).toHaveBeenCalledOnce());

    native.progress.mockResolvedValueOnce(INBOUND);
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
    });

    await waitFor(() => expect(useUi.getState().view).toBe("devices"));
  });

  it("keeps the 700 ms poll only while a pairing ceremony is active", () => {
    expect(pairingProgressInterval(IDLE)).toBe(false);
    expect(pairingProgressInterval(INBOUND)).toBe(PAIRING_POLL_MS);
    expect(PAIRING_POLL_MS).toBe(700);
  });
});
