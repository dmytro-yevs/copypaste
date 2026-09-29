import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { withClient } from "@/test/harness";
import type { OnboardingPermissions as Snapshot } from "@/lib/ipc";
import { OnboardingPermissions } from "./OnboardingPermissions";

const ipc = vi.hoisted(() => ({ snapshot: vi.fn(), request: vi.fn(), settings: vi.fn(), startup: vi.fn(), saveStartup: vi.fn() }));
vi.mock("@/lib/ipc", async (original) => ({
  ...await original<typeof import("@/lib/ipc")>(),
  permissionSnapshot: () => ipc.snapshot(), permissionRequest: (id: string) => ipc.request(id),
  permissionOpenSettings: (id: string) => ipc.settings(id), getOpenAtLogin: () => ipc.startup(), setOpenAtLogin: (value: boolean) => ipc.saveStartup(value),
}));
vi.mock("@/lib/ipcCall", async (original) => ({ ...await original<typeof import("@/lib/ipcCall")>(), hasNativeBridge: () => true }));
const snapshot: Snapshot = {
  platform: "android", notifications: { id: "notifications", status: "prompt", required: false },
  tile: { id: "tile", status: "prompt", required: false }, clipboardStatus: "not_required",
  backgroundActivity: { id: "background_activity", status: "prompt", required: false },
};
beforeEach(() => {
  vi.resetAllMocks();
  ipc.snapshot.mockResolvedValue(snapshot);
  ipc.request.mockResolvedValue(snapshot);
  ipc.settings.mockResolvedValue(snapshot);
  ipc.startup.mockResolvedValue(false);
  ipc.saveStartup.mockResolvedValue(true);
});

it("offers notifications and real battery permission on Android without startup or tile controls", async () => {
  withClient(<OnboardingPermissions android />);
  fireEvent.click(await screen.findByRole("button", { name: "Notifications: Allow" }));
  await waitFor(() => expect(ipc.request).toHaveBeenCalledWith("notifications"));
  await waitFor(() => expect(screen.getByRole("button", { name: "Background activity: Allow" }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "Background activity: Allow" }));
  await waitFor(() => expect(ipc.request).toHaveBeenCalledWith("background_activity"));
  expect(screen.queryByRole("switch")).toBeNull();
  expect(screen.queryByText("Quick Settings tile")).toBeNull();
  expect(ipc.startup).not.toHaveBeenCalled();
});

it("opens system settings for denied notifications", async () => {
  ipc.snapshot.mockResolvedValue({ ...snapshot, notifications: { ...snapshot.notifications, status: "denied" } });
  withClient(<OnboardingPermissions android />);
  fireEvent.click(await screen.findByRole("button", { name: "Notifications: Open settings" }));
  await waitFor(() => expect(ipc.settings).toHaveBeenCalledWith("notifications"));
  expect(ipc.request).not.toHaveBeenCalled();
});

it.each(["macos", "windows"])("uses native startup and needs no background grant on %s", async (platform) => {
  ipc.snapshot.mockResolvedValue({ ...snapshot, platform, backgroundActivity: { ...snapshot.backgroundActivity, status: "not_required" } });
  withClient(<OnboardingPermissions android={false} />);
  expect(await screen.findByText("No permission needed")).toBeTruthy();
  const startup = screen.getByRole("switch", { name: "Start at launch" });
  await waitFor(() => expect(startup.hasAttribute("disabled")).toBe(false));
  fireEvent.click(startup);
  await waitFor(() => expect(ipc.saveStartup).toHaveBeenCalledWith(true));
  expect(screen.queryByRole("button", { name: /Background activity/ })).toBeNull();
});

it("shows permission check errors without pretending permission was granted", async () => {
  ipc.snapshot.mockRejectedValueOnce(new Error("unavailable"));
  withClient(<OnboardingPermissions android />);
  expect(await screen.findByRole("alert")).toBeTruthy();
  expect(screen.queryByText("Allowed")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  await screen.findByRole("button", { name: "Notifications: Allow" });
});

it("rechecks background permission after returning from Android settings", async () => {
  withClient(<OnboardingPermissions android />);
  await screen.findByRole("button", { name: "Background activity: Allow" });
  ipc.snapshot.mockResolvedValue({ ...snapshot, backgroundActivity: { ...snapshot.backgroundActivity, status: "granted" } });
  fireEvent.focus(window);
  expect(await screen.findByText("Allowed")).toBeTruthy();
});
