import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { peer } from "@/test/harness";
import { useUi } from "@/store/ui";
import { DevicesScreen } from "./DevicesScreen";

const mocks = vi.hoisted(() => ({
  peers: undefined as unknown,
  config: undefined as unknown,
  servicePending: false,
  serviceError: false,
  peerPending: false,
  peerError: false,
  capture: undefined as unknown,
  syncPending: false,
  mutate: vi.fn(),
  refetchService: vi.fn(),
  refetchConfig: vi.fn(),
  refetchPeers: vi.fn(),
}));

vi.mock("@/hooks/useViewportMetrics", () => ({
  useViewportMetrics: () => ({ width: 1_024, sizeClass: "expanded" }),
  useObservedElementSize: () => ({ ref: () => {}, width: 1_024, height: 800 }),
}));

vi.mock("@/hooks/useStatus", () => ({
  useStatus: () => ({
    data: { device_name: "This device", capture_running: true, private_mode: false },
    isPending: mocks.servicePending,
    isError: mocks.serviceError,
    refetch: mocks.refetchService,
  }),
}));

vi.mock("@/hooks/useCapture", () => ({
  useCaptureState: () => ({ data: mocks.capture }),
}));

vi.mock("@/hooks/useServiceConfig", () => ({
  useServiceConfig: () => ({ data: mocks.config, isPending: false, isError: false, refetch: mocks.refetchConfig }),
}));

vi.mock("@/hooks/useDevices", () => ({
  usePeers: () => ({ data: mocks.peers, isPending: mocks.peerPending, isError: mocks.peerError, refetch: mocks.refetchPeers }),
  useDiscovered: () => ({ data: [], isPending: false, isError: false }),
  useRescan: () => ({ mutate: vi.fn(), isPending: false }),
  useRevoke: () => ({ mutateAsync: vi.fn(), isPending: false, error: null }),
  useUnpair: () => ({ mutateAsync: vi.fn(), isPending: false, error: null }),
  useSyncNow: () => ({ mutate: mocks.mutate, isPending: mocks.syncPending, variables: undefined }),
}));

vi.mock("@/hooks/useCloud", () => ({
  useCloudStatus: () => ({ data: undefined, isPending: false, isError: false }),
}));

vi.mock("@/features/pairing", () => ({
  usePairing: () => ({ ceremony: null, error: null, isChecking: false, isPending: false, run: vi.fn() }),
}));

vi.mock("@/features/devices/patterns/useDeviceDetailTarget", () => ({
  selectDeviceStatus: (value: unknown) => value,
  useDeviceDetailTarget: () => {
    const selected = (mocks.peers as ReturnType<typeof peer>[] | undefined)?.[0];
    return selected ? { kind: "peer", peer: selected, name: "Kitchen Mac" } : null;
  },
}));

vi.mock("@/features/devices/patterns/DeviceDetailPane", () => ({
  DeviceDetailPane: ({ target, onSync, onRecoverSync }: {
    target: { peer: unknown };
    onSync: (peer: unknown) => void;
    onRecoverSync: () => void;
  }) => <>
    <button onClick={() => onSync(target.peer)}>Force detail sync</button>
    <button onClick={onRecoverSync}>Recover detail sync</button>
  </>,
}));

vi.mock("@/features/devices/components/ConnectionSummary", () => ({
  ConnectionSummary: ({ onAction }: { onAction: () => void }) =>
    <button onClick={onAction}>Force summary retry</button>,
}));

vi.mock("@/features/devices/model/devicePresentation", async (importOriginal) => ({
  ...await importOriginal<typeof import("@/features/devices/model/devicePresentation")>(),
  connectionSummary: () => ({
    title: "Retry peer",
    busy: false,
    status: "attention",
    icon: "alert",
    live: "polite",
    action: { kind: "retry-peer", label: "Try again", icon: "refresh", pairingId: "pair-1" },
  }),
}));

beforeEach(() => {
  mocks.peers = [peer()];
  mocks.config = { config: { sync_enabled: true } };
  mocks.servicePending = false;
  mocks.serviceError = false;
  mocks.peerPending = false;
  mocks.peerError = false;
  mocks.capture = undefined;
  mocks.syncPending = false;
  mocks.mutate.mockReset();
  mocks.refetchService.mockReset();
  mocks.refetchConfig.mockReset();
  mocks.refetchPeers.mockReset();
  useUi.setState({ view: "devices", settingsTab: null });
});

describe("Devices sync readiness", () => {
  it("uses the canonical capture snapshot instead of inferring capture state", () => {
    mocks.capture = {
      headline: "Background capture needs setup.",
      health: { state: "not_granted", reason: "no_permission" },
    };
    render(<TooltipProvider><DevicesScreen /></TooltipProvider>);

    expect(screen.getByRole("button", {
      name: /This device\. This device\. Background capture needs setup\./,
    })).toBeTruthy();
  });

  it("allows detail and retry syncs only when the master setting is ready", () => {
    render(<TooltipProvider><DevicesScreen /></TooltipProvider>);

    fireEvent.click(screen.getByRole("button", { name: "Force summary retry" }));
    fireEvent.click(screen.getByRole("button", { name: /Kitchen Mac\. Device name is self-reported/ }));
    fireEvent.click(screen.getByRole("button", { name: "Force detail sync" }));
    expect(mocks.mutate).toHaveBeenCalledTimes(2);
    expect(mocks.mutate).toHaveBeenNthCalledWith(1, "pair-1", expect.any(Object));
    expect(mocks.mutate).toHaveBeenNthCalledWith(2, "pair-1", expect.any(Object));
  });

  it("guards every sync path while disabled and opens its actual setting", () => {
    mocks.config = { config: { sync_enabled: false } };
    render(<TooltipProvider><DevicesScreen /></TooltipProvider>);

    expect(screen.getByText("Sync with paired devices is off.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Force summary retry" }));
    fireEvent.click(screen.getByRole("button", { name: "Open Device sync settings" }));
    expect(useUi.getState()).toMatchObject({ view: "settings", settingsTab: "device-sync" });
    fireEvent.click(screen.getByRole("button", { name: /Kitchen Mac\. Device name is self-reported/ }));
    fireEvent.click(screen.getByRole("button", { name: "Force detail sync" }));
    expect(mocks.mutate).not.toHaveBeenCalled();
  });

  it("does not start a second run while one is pending", () => {
    mocks.syncPending = true;
    render(<TooltipProvider><DevicesScreen /></TooltipProvider>);

    fireEvent.click(screen.getByRole("button", { name: "Force summary retry" }));
    fireEvent.click(screen.getByRole("button", { name: /Kitchen Mac\. Device name is self-reported/ }));
    fireEvent.click(screen.getByRole("button", { name: "Force detail sync" }));
    expect(mocks.mutate).not.toHaveBeenCalled();
  });

  it("reports service unavailability before config and retries the service", () => {
    mocks.serviceError = true;
    mocks.config = undefined;
    render(<TooltipProvider><DevicesScreen /></TooltipProvider>);

    expect(screen.getByText("The clipboard service is unavailable.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(mocks.refetchService).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: /Kitchen Mac\. Device name is self-reported/ }));
    fireEvent.click(screen.getByRole("button", { name: "Force detail sync" }));
    expect(mocks.mutate).not.toHaveBeenCalled();
  });

  it.each(["config", "peers"] as const)("retries the failed %s read", (source) => {
    if (source === "config") mocks.config = undefined;
    else mocks.peerError = true;
    render(<TooltipProvider><DevicesScreen /></TooltipProvider>);

    expect(screen.getByText(source === "config"
      ? "The device sync setting is unavailable."
      : "Paired devices are unavailable.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(source === "config" ? mocks.refetchConfig : mocks.refetchPeers).toHaveBeenCalledOnce();
    expect(mocks.mutate).not.toHaveBeenCalled();
  });

  it("uses the same config recovery from the notice and device detail", () => {
    mocks.config = undefined;
    render(<TooltipProvider><DevicesScreen /></TooltipProvider>);

    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    fireEvent.click(screen.getByRole("button", { name: /Kitchen Mac\. Device name is self-reported/ }));
    fireEvent.click(screen.getByRole("button", { name: "Recover detail sync" }));
    expect(mocks.refetchConfig).toHaveBeenCalledTimes(2);
  });

  it("routes detail recovery to Device sync settings when disabled", () => {
    mocks.config = { config: { sync_enabled: false } };
    render(<TooltipProvider><DevicesScreen /></TooltipProvider>);

    fireEvent.click(screen.getByRole("button", { name: /Kitchen Mac\. Device name is self-reported/ }));
    fireEvent.click(screen.getByRole("button", { name: "Recover detail sync" }));
    expect(useUi.getState()).toMatchObject({ view: "settings", settingsTab: "device-sync" });
  });

  it("does not add a no-peers status card beside the pairing action", () => {
    mocks.peers = [];
    render(<TooltipProvider><DevicesScreen /></TooltipProvider>);

    expect(screen.queryByText("Pair a device before syncing.")).toBeNull();
    expect(mocks.mutate).not.toHaveBeenCalled();
  });
});
