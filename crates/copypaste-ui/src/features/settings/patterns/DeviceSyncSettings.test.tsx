import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { peer } from "@/test/harness";
import { useUi } from "@/store/ui";
import { DeviceSyncSettings } from "./DeviceSyncSettings";

const mocks = vi.hoisted(() => ({
  peers: undefined as unknown,
  peersPending: false,
  peersError: false,
  config: undefined as unknown,
  configPending: false,
  configError: false,
  servicePending: false,
  serviceError: false,
  syncPending: false,
  syncError: false,
  syncData: undefined as unknown,
  mutate: vi.fn(),
}));

vi.mock("@/features/devices", () => ({
  DeviceNameField: () => <span>Local device</span>,
}));

vi.mock("@/hooks/useDevices", () => ({
  usePeers: () => ({ data: mocks.peers, isPending: mocks.peersPending, isError: mocks.peersError }),
  useSyncNow: () => ({
    mutate: mocks.mutate,
    isPending: mocks.syncPending,
    isError: mocks.syncError,
    data: mocks.syncData,
  }),
}));

vi.mock("@/hooks/useServiceConfig", () => ({
  useServiceConfig: () => ({ data: mocks.config, isPending: mocks.configPending, isError: mocks.configError }),
}));

vi.mock("@/hooks/useStatus", () => ({
  statusReachable: () => true,
  useStatus: () => ({ isPending: mocks.servicePending, isError: mocks.serviceError }),
}));

beforeEach(() => {
  mocks.peers = [peer({ last_seen_ms: 1_700_000_000_000 })];
  mocks.peersPending = false;
  mocks.peersError = false;
  mocks.config = { config: { sync_enabled: true } };
  mocks.configPending = false;
  mocks.configError = false;
  mocks.servicePending = false;
  mocks.serviceError = false;
  mocks.syncPending = false;
  mocks.syncError = false;
  mocks.syncData = undefined;
  mocks.mutate.mockReset();
  useUi.setState({ view: "settings", settingsTab: null });
});

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
    expect(screen.getByRole("button", { name: "Syncing…" }).hasAttribute("disabled")).toBe(true);
  });

  it("blocks manual sync when the service setting is off and opens its control", () => {
    mocks.config = { config: { sync_enabled: false } };
    render(<DeviceSyncSettings />);

    expect(screen.getByRole("button", { name: "Sync now" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByText(/Device sync is off/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Open Service settings" }));
    expect(useUi.getState().settingsTab).toBe("service");
    expect(mocks.mutate).not.toHaveBeenCalled();
  });

  it("keeps manual sync unavailable until its configuration is known", () => {
    mocks.config = undefined;
    mocks.configPending = true;
    const { rerender } = render(<DeviceSyncSettings />);
    expect(screen.getByText(/Checking whether device sync is on/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sync now" }).hasAttribute("disabled")).toBe(true);

    mocks.configPending = false;
    mocks.configError = true;
    rerender(<DeviceSyncSettings />);
    expect(screen.getByText(/couldn't read the device sync setting/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open Service settings" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sync now" }).hasAttribute("disabled")).toBe(true);
  });

  it("handles no pairings and a failed peer read without reporting connection health", () => {
    mocks.peers = [];
    const { rerender } = render(<DeviceSyncSettings />);
    expect(screen.getByText("None")).toBeTruthy();
    expect(screen.getByText("Pair a device before syncing.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sync now" }).hasAttribute("disabled")).toBe(true);

    mocks.peers = undefined;
    mocks.peersError = true;
    rerender(<DeviceSyncSettings />);
    expect(screen.getByText("Unavailable")).toBeTruthy();
    expect(screen.getByText("CopyPaste couldn't read paired devices.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Manage devices" })).toBeTruthy();
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
