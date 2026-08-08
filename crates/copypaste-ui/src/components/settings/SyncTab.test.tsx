/**
 * The tab has three rows and two of them are about *peer* sync, so the one
 * thing worth pinning is that they stay distinguishable:
 *
 * The cloud row's badge is a standing fact about the build. The paired row's is
 * a report on a call that failed. They read the same word and used to be the
 * same catalogue key, which meant a service that could not answer for peers
 * rendered a sentence about accounts.
 */
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

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    listPeers: () => listPeers(),
    syncNow: (pairingId?: string) => syncNow(pairingId),
    getStatus: () => getStatus(),
    setDeviceName: (name: string) => setDeviceName(name),
  };
});

beforeEach(() => {
  listPeers.mockReset().mockResolvedValue([peer()]);
  syncNow.mockReset().mockResolvedValue([]);
  getStatus.mockReset().mockResolvedValue(status());
  setDeviceName.mockReset().mockResolvedValue(undefined);
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

  /** Both rows read "Unavailable" here, and they are saying different things:
   *  the paired row is reporting this call, the cloud row is stating a fact
   *  about the build. They are separate catalogue keys for that reason. */
  it("reports a peer list the service could not answer for", async () => {
    listPeers.mockRejectedValue(new IpcFailure("unavailable", false));
    withUser(<SyncTab />);

    await waitFor(() =>
      expect(
        screen.getAllByText(en.settings.sync.paired.unavailable),
      ).toHaveLength(2),
    );
    expect(
      screen.getByRole("button", { name: en.settings.sync.now.action }),
    ).toHaveProperty("disabled", true);
  });

  /** A badge with nothing beside it says only that something is missing. */
  it("says why cloud sync is not there", async () => {
    withUser(<SyncTab />);

    expect(await screen.findByText(en.settings.sync.cloud.title)).not.toBeNull();
    expect(screen.getByText(en.settings.sync.cloud.description)).not.toBeNull();
    expect(screen.getByText(en.settings.sync.cloud.badge)).not.toBeNull();
  });
});
