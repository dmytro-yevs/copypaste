import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { SyncTab } from "@/components/settings/SyncTab";
import { en } from "@/i18n";
import { IpcFailure } from "@/lib/errors";
import { peer, status, withUser } from "@/test/harness";

const listPeers = vi.fn();
const syncNow = vi.fn();
const getStatus = vi.fn();
const setDeviceName = vi.fn();
const getCloudStatus = vi.fn();
const cloudSignIn = vi.fn();
const cloudSignOut = vi.fn();
const syncCloudNow = vi.fn();

function cloudStatus(overrides = {}) {
  return {
    configured: false,
    signed_in: false,
    key_ready: false,
    email: null,
    last_sync_ms: null,
    last_error: null,
    poll_interval_secs: 60,
    unreadable_uploads: 0,
    ...overrides,
  };
}

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    listPeers: () => listPeers(),
    syncNow: (pairingId?: string) => syncNow(pairingId),
    getStatus: () => getStatus(),
    setDeviceName: (name: string) => setDeviceName(name),
    getCloudStatus: () => getCloudStatus(),
    cloudSignIn: (credentials: unknown) => cloudSignIn(credentials),
    cloudSignOut: () => cloudSignOut(),
    syncCloudNow: () => syncCloudNow(),
  };
});

beforeEach(() => {
  listPeers.mockReset().mockResolvedValue([peer()]);
  syncNow.mockReset().mockResolvedValue([]);
  getStatus.mockReset().mockResolvedValue(status());
  setDeviceName.mockReset().mockResolvedValue(undefined);
  getCloudStatus.mockReset().mockResolvedValue(cloudStatus());
  cloudSignIn.mockReset().mockResolvedValue(cloudStatus({ configured: true, signed_in: true, key_ready: true }));
  cloudSignOut.mockReset().mockResolvedValue(cloudStatus({ configured: true }));
  syncCloudNow.mockReset().mockResolvedValue({
    uploaded: 1,
    tombstoned: 0,
    downloaded: 2,
    applied: 2,
    skipped_sensitive: 0,
    skipped_undecryptable: 0,
    skipped_forged: 0,
    skipped_future: 0,
    skipped_too_large: 0,
  });
});

describe("SyncTab", () => {
  it("counts the paired devices and offers to sync them", async () => {
    const { user } = withUser(<SyncTab />);

    expect(await screen.findByText("1 paired")).not.toBeNull();
    await user.click(screen.getByRole("button", { name: en.settings.sync.now.action }));
    expect(syncNow).toHaveBeenCalledTimes(1);
  });

  it("keeps device naming available without exposing pairing controls", async () => {
    const { user } = withUser(<SyncTab />);
    const input = await screen.findByRole("textbox", { name: "Device name" });
    await waitFor(() => expect(input).toHaveProperty("disabled", false));

    await user.clear(input);
    await user.type(input, "Office Phone");
    await user.click(screen.getByRole("button", { name: "Rename" }));

    await waitFor(() => expect(setDeviceName).toHaveBeenCalledWith("Office Phone"));
    expect(screen.queryByRole("button", { name: /^pair$/i })).toBeNull();
    expect(screen.queryByRole("button", { name: /^add device$/i })).toBeNull();
  });

  it("will not offer to sync nothing", async () => {
    listPeers.mockResolvedValue([]);
    withUser(<SyncTab />);

    expect(await screen.findByText(en.settings.sync.paired.none)).not.toBeNull();
    expect(
      screen.getByRole("button", { name: en.settings.sync.now.action }),
    ).toHaveProperty("disabled", true);
  });

  it("reports a peer list the service could not answer for", async () => {
    listPeers.mockRejectedValue(new IpcFailure("unavailable", false));
    withUser(<SyncTab />);

    expect(await screen.findByText(en.settings.sync.paired.unavailable)).not.toBeNull();
    expect(
      screen.getByRole("button", { name: en.settings.sync.now.action }),
    ).toHaveProperty("disabled", true);
  });

  it("explains when this build has no cloud deployment", async () => {
    withUser(<SyncTab />);

    expect(await screen.findByText(en.settings.sync.cloud.title)).not.toBeNull();
    expect(screen.getByText(en.settings.sync.cloud.notConfigured)).not.toBeNull();
    expect(screen.getByText(en.settings.sync.cloud.badgeNotConfigured)).not.toBeNull();
    expect(screen.queryByRole("button", { name: en.settings.sync.cloud.signIn })).toBeNull();
  });

  it.each([
    ["leading", " password"],
    ["trailing", "password "],
    ["whitespace-only", "   "],
  ])("preserves a %s-space password exactly", async (_kind, password) => {
    getCloudStatus.mockResolvedValue(cloudStatus({ configured: true }));
    const { user } = withUser(<SyncTab />);

    await user.type(await screen.findByLabelText(en.settings.sync.cloud.email), " me@example.com ");
    await user.type(screen.getByLabelText(en.settings.sync.cloud.password), password);
    await user.type(screen.getByLabelText(en.settings.sync.cloud.passphrase), "phrase with spaces");
    await user.click(screen.getByRole("button", { name: en.settings.sync.cloud.signIn }));

    await waitFor(() =>
      expect(cloudSignIn).toHaveBeenCalledWith({
        email: "me@example.com",
        password,
        passphrase: "phrase with spaces",
      }),
    );
    expect(screen.queryByRole("button", { name: /^pair$/i })).toBeNull();
  });

  // A row this device cannot open is not going to the account, and sync is
  // otherwise working — so nothing else on this screen would say so. It is
  // announced (`role="status"`) rather than only drawn, and it never carries
  // the row's identity or content.
  it("says how many items could not be prepared for cloud sync", async () => {
    getCloudStatus.mockResolvedValue(
      cloudStatus({
        configured: true,
        signed_in: true,
        key_ready: true,
        email: "me@example.com",
        unreadable_uploads: 3,
      }),
    );
    withUser(<SyncTab />);

    const note = await screen.findByText(/could not be prepared for cloud sync/);
    expect(note.getAttribute("role")).toBe("status");
    expect(note.textContent).toContain("3 items");
    expect(screen.queryByText(en.settings.sync.cloud.lastError)).toBeNull();
  });

  it("says nothing about skipped items when there are none", async () => {
    getCloudStatus.mockResolvedValue(
      cloudStatus({ configured: true, signed_in: true, key_ready: true }),
    );
    withUser(<SyncTab />);

    expect(await screen.findByText(en.settings.sync.cloud.badgeConnected)).not.toBeNull();
    expect(screen.queryByText(/could not be prepared for cloud sync/)).toBeNull();
  });

  it("offers manual sync and sign out for an unlocked account", async () => {
    getCloudStatus.mockResolvedValue(
      cloudStatus({
        configured: true,
        signed_in: true,
        key_ready: true,
        email: "me@example.com",
      }),
    );
    const { user } = withUser(<SyncTab />);

    await user.click(await screen.findByRole("button", { name: en.settings.sync.cloud.syncNow }));
    await waitFor(() => expect(syncCloudNow).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole("button", { name: en.settings.sync.cloud.signOut }));
    await waitFor(() => expect(cloudSignOut).toHaveBeenCalledTimes(1));
  });
});
