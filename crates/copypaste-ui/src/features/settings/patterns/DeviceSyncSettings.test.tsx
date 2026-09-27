import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { SettingsScreen } from "@/features/settings/screen/SettingsScreen";
import { peer } from "@/test/harness";
import { useUi } from "@/store/ui";
import { DeviceSyncSettings } from "./DeviceSyncSettings";

const mocks = vi.hoisted(() => ({
  peers: undefined as unknown,
  peersPending: false,
  peersError: false,
  peersRefetch: vi.fn(),
  config: undefined as unknown,
  configPending: false,
  configError: false,
  servicePending: false,
  serviceError: false,
  serviceData: true as unknown,
  serviceRefetch: vi.fn(),
  syncPending: false,
  syncError: false,
  syncData: undefined as unknown,
  mutate: vi.fn(),
  refetch: vi.fn(),
}));

vi.mock("@/hooks/useViewportMetrics", () => ({
  useViewportMetrics: () => ({ width: 1_024, height: 800 }),
  useViewportSizeClass: () => "expanded",
  useObservedElementSize: () => ({ ref: () => {}, width: 1_024, height: 800 }),
}));

vi.mock("@/features/devices", () => ({
  DeviceNameField: () => <span>Local device</span>,
}));

vi.mock("@/hooks/useDevices", () => ({
  usePeers: () => ({ data: mocks.peers, isPending: mocks.peersPending, isError: mocks.peersError, refetch: mocks.peersRefetch }),
  useSyncNow: () => ({
    mutate: mocks.mutate,
    isPending: mocks.syncPending,
    isError: mocks.syncError,
    data: mocks.syncData,
  }),
}));

vi.mock("@/hooks/useServiceConfig", () => ({
  useServiceConfig: () => ({ data: mocks.config, isPending: mocks.configPending, isError: mocks.configError, refetch: mocks.refetch }),
  useSetServiceConfig: () => ({ mutateAsync: vi.fn(), isPending: false, isError: false }),
  usePrivateMode: () => ({ data: undefined, isPending: false, isError: false }),
  useSetPrivateMode: () => ({ mutate: vi.fn(), isPending: false, isError: false }),
  useRestartService: () => ({ mutate: vi.fn(), isPending: false }),
}));

vi.mock("@/hooks/useStatus", () => ({
  statusReachable: () => true,
  useStatus: () => ({ data: mocks.serviceData, isPending: mocks.servicePending, isError: mocks.serviceError, refetch: mocks.serviceRefetch }),
}));

beforeEach(() => {
  mocks.peers = [peer({ last_seen_ms: 1_700_000_000_000 })];
  mocks.peersPending = false;
  mocks.peersError = false;
  mocks.peersRefetch.mockReset();
  mocks.config = { config: { sync_enabled: true } };
  mocks.configPending = false;
  mocks.configError = false;
  mocks.servicePending = false;
  mocks.serviceError = false;
  mocks.serviceData = true;
  mocks.serviceRefetch.mockReset();
  mocks.syncPending = false;
  mocks.syncError = false;
  mocks.syncData = undefined;
  mocks.mutate.mockReset();
  mocks.refetch.mockReset();
  useUi.setState({ view: "settings", settingsTab: null });
});

afterEach(() => {
  delete (Element.prototype as Partial<Element>).scrollIntoView;
});

function expectDisabledSyncDescribes(reason: string) {
  const button = screen.getByRole("button", { name: "Sync now" });
  expect(button.hasAttribute("disabled")).toBe(true);
  const ids = button.getAttribute("aria-describedby")?.split(" ") ?? [];
  expect(ids.some((id) => document.getElementById(id)?.textContent?.includes(reason))).toBe(true);
}

describe("Device sync settings", () => {
  it("shows a pairing count without implying a current connection", () => {
    render(<DeviceSyncSettings />);

    expect(screen.getByText("1 paired")).toBeTruthy();
    expect(screen.queryByText(/^(Connected|Synced)$/i)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Sync now" }));
    expect(mocks.mutate).toHaveBeenCalledWith(undefined, expect.any(Object));
  });

  it("uses the Devices summary while a manual sync is running", () => {
    mocks.syncPending = true;
    render(<DeviceSyncSettings />);

    expect(screen.getByText(/Syncing your devices/)).toBeTruthy();
    const button = screen.getByRole("button", { name: "Syncing…" });
    expect(button.hasAttribute("disabled")).toBe(true);
    const ids = button.getAttribute("aria-describedby")?.split(" ") ?? [];
    expect(ids.some((id) => document.getElementById(id)?.textContent?.includes("Syncing your devices"))).toBe(true);
  });

  it("blocks manual sync when the service setting is off and requests the correct section", () => {
    mocks.config = { config: { sync_enabled: false } };
    render(<DeviceSyncSettings />);

    expect(screen.getByRole("button", { name: "Sync now" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByText(/Sync with paired devices is off/)).toBeTruthy();
    expectDisabledSyncDescribes("Sync with paired devices is off");
    fireEvent.click(screen.getByRole("button", { name: "Show sync setting" }));
    expect(useUi.getState().settingsTab).toBe("device-sync");
    expect(mocks.mutate).not.toHaveBeenCalled();
  });

  it("keeps manual sync unavailable until its configuration is known", () => {
    mocks.config = undefined;
    mocks.configPending = true;
    const { rerender } = render(<DeviceSyncSettings />);
    expect(screen.getByText(/Checking whether device sync is on/)).toBeTruthy();
    expectDisabledSyncDescribes("Checking whether device sync is on");

    mocks.configPending = false;
    mocks.configError = true;
    rerender(<DeviceSyncSettings />);
    expect(screen.getByText(/device sync setting is unavailable/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(mocks.refetch).toHaveBeenCalledOnce();
    expectDisabledSyncDescribes("The device sync setting is unavailable");
  });

  it("shows service loading and service failure without hiding a known pairing count", () => {
    mocks.serviceData = undefined;
    mocks.servicePending = true;
    const { rerender } = render(<DeviceSyncSettings />);
    expect(screen.getByText("1 paired")).toBeTruthy();
    expectDisabledSyncDescribes("Checking the clipboard service before syncing");

    mocks.serviceData = true;
    mocks.servicePending = false;
    mocks.serviceError = true;
    rerender(<DeviceSyncSettings />);
    expect(screen.getByText("1 paired")).toBeTruthy();
    expectDisabledSyncDescribes("The clipboard service is unavailable");
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(mocks.serviceRefetch).toHaveBeenCalledOnce();
    expect(mocks.mutate).not.toHaveBeenCalled();
  });

  it("names peer loading and peer failure separately from service health", () => {
    mocks.peers = undefined;
    mocks.peersPending = true;
    const { rerender } = render(<DeviceSyncSettings />);
    expect(screen.getByText("Checking…")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Sync now" })).toBeNull();

    mocks.peersPending = false;
    mocks.peersError = true;
    rerender(<DeviceSyncSettings />);
    expect(screen.getByText("Unavailable")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Sync now" })).toBeNull();

    mocks.peers = [peer()];
    rerender(<DeviceSyncSettings />);
    expect(screen.getByText("1 paired last known")).toBeTruthy();
    expectDisabledSyncDescribes("Paired devices are unavailable");
  });

  it("focuses the actual sync switch inside SettingsScreen", async () => {
    mocks.config = { config: { sync_enabled: false, lan_visibility: true }, restart_required: [] };
    const scroll = vi.fn();
    Object.defineProperty(Element.prototype, "scrollIntoView", { configurable: true, value: scroll });
    useUi.setState({ settingsTab: "device-sync" });
    render(
      <TooltipProvider>
        <SettingsScreen />
      </TooltipProvider>,
    );

    const action = await screen.findByRole("button", { name: "Show sync setting" });
    await waitFor(() => expect(useUi.getState().settingsTab).toBeNull());
    const target = screen.getByRole("switch", { name: "Sync with paired devices" });
    expect(document.activeElement).not.toBe(target);
    fireEvent.click(action);

    expect(document.activeElement).toBe(target);
    expect(scroll).toHaveBeenCalled();
    expect(target.closest("[data-settings-search-target]")?.getAttribute("data-settings-search-target"))
      .toBe("row:Sync with paired devices");
  });

  it("handles no pairings and a failed peer read without reporting connection health", () => {
    mocks.peers = [];
    const { rerender } = render(<DeviceSyncSettings />);
    expect(screen.getByText("None")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Sync now" })).toBeNull();

    mocks.peers = undefined;
    mocks.peersError = true;
    rerender(<DeviceSyncSettings />);
    expect(screen.getByText("Unavailable")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Sync now" })).toBeNull();
    expect(screen.getByRole("button", { name: "Open" })).toBeTruthy();
  });

  it("shows manual failures and partial results without a success claim", () => {
    mocks.syncError = true;
    const { rerender } = render(<DeviceSyncSettings />);
    expect(screen.getByText("Sync couldn't start.")).toBeTruthy();

    mocks.syncError = false;
    mocks.syncData = [{ pairing_id: "pair-1", error: { code: "peer_failed" } }];
    rerender(<DeviceSyncSettings />);
    expect(screen.getByText(/Some devices couldn't sync/)).toBeTruthy();
    expect(screen.queryByText(/^(Connected|Synced)$/i)).toBeNull();
  });

  it("uses a recorded peer failure for a specific recovery summary", () => {
    const { rerender } = render(<DeviceSyncSettings />);
    fireEvent.click(screen.getByRole("button", { name: "Sync now" }));
    const results = [{
      pairing_id: "pair-1",
      sent: 0,
      received: 0,
      duration_ms: 10,
      error: { code: "peer_failed", retryable: true, message: "Peer sync failed" },
    }];
    const options = mocks.mutate.mock.calls[0]?.[1];
    options.onSuccess(results);
    mocks.syncData = results;
    rerender(<DeviceSyncSettings />);

    expect(screen.getByText(/Sync with Kitchen Mac failed/)).toBeTruthy();
  });
});
