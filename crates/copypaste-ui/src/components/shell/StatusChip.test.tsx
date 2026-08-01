import { beforeEach, describe, expect, it, vi } from "vitest";
import { configure, screen, waitFor } from "@testing-library/react";

import { StatusChip } from "@/components/shell/StatusChip";
import { IpcFailure } from "@/lib/errors";
import { useUi } from "@/store/ui";
import { peer, status, withClient, withUser } from "@/test/harness";

const getStatus = vi.fn();
const listPeers = vi.fn();

configure({ asyncUtilTimeout: 10_000 });

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    getStatus: () => getStatus(),
    listPeers: () => listPeers(),
  };
});

beforeEach(() => {
  getStatus.mockReset().mockResolvedValue(status());
  listPeers.mockReset().mockResolvedValue([
    peer({ last_seen_ms: Date.now(), online: true }),
  ]);
  useUi.setState({ view: "history", settingsTab: null });
});

describe("sync status", () => {
  it("reports peer state in the visible and accessible tooltip", async () => {
    withClient(<StatusChip />);

    const chip = await screen.findByRole("status", { name: /sync: synced/i });
    expect(chip.getAttribute("title")).toMatch(/1 paired device/i);
    expect(chip.getAttribute("title")).toMatch(/last peer sync just now/i);

    const tooltip = screen.getByRole("tooltip", { hidden: true });
    expect(chip.getAttribute("aria-describedby")).toBe(tooltip.id);
    expect(tooltip.textContent).toBe(chip.getAttribute("title"));
  });

  it("reports the binding thirty-minute peer stall in the global state", async () => {
    listPeers.mockResolvedValue([
      peer({ last_seen_ms: Date.now() - 30 * 60_000 - 1, online: false }),
    ]);
    withClient(<StatusChip />);

    const chip = await screen.findByRole("status", {
      name: /sync: needs attention/i,
    });
    expect(chip.getAttribute("aria-label")).toMatch(/1 peer not syncing/i);
  });

  it("distinguishes an unreachable service from a sync backend error", async () => {
    getStatus.mockRejectedValue(new IpcFailure("offline", true));
    listPeers.mockRejectedValue(new IpcFailure("offline", true));
    withClient(<StatusChip />);

    const chip = await screen.findByRole("status", { name: /sync: offline/i });
    expect(chip.getAttribute("aria-label")).toMatch(
      /background service unreachable/i,
    );
  });

  it("makes persistent service trouble a compact route to recovery", async () => {
    getStatus.mockRejectedValue(new IpcFailure("offline", true));
    listPeers.mockResolvedValue([]);
    const { user } = withUser(<StatusChip />);

    const chip = await screen.findByRole("status", { name: /sync: offline/i });
    const action = screen.getByRole("button", { name: /open service settings/i });
    expect(chip.querySelector("button")?.className).toContain("rounded-full");

    await user.click(action);
    expect(useUi.getState().view).toBe("settings");
    expect(useUi.getState().settingsTab).toBe("service");
  });

  it("does not add an Android floating status affordance while healthy", async () => {
    withClient(<StatusChip attentionOnly />);
    await waitFor(() => expect(screen.queryByRole("status")).toBeNull());
  });
});
