import { beforeEach, describe, expect, it, vi } from "vitest";
import { configure, screen, waitFor } from "@testing-library/react";

import { StatusChip } from "@/components/shell/StatusChip";
import { IpcFailure } from "@/lib/errors";
import { STATUS_POLL_MS } from "@/lib/layout";
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

  /** `CopyPaste-f701`: a 10s poll left this chip green long after the service
   *  had died. F-UI-3 narrows what the chip *selects* out of the status payload
   *  so a ticking uptime stops re-rendering the app — the liveness signal it
   *  reads is `status.error`, and that must still arrive within one poll. */
  it("goes red within one poll of the service dying", async () => {
    withClient(<StatusChip />);
    await screen.findByRole("status", { name: /sync: synced/i });

    getStatus.mockRejectedValue(new IpcFailure("offline", true));
    await waitFor(
      () => expect(screen.getByRole("status").getAttribute("aria-label")).toMatch(
        /sync: offline/i,
      ),
      { timeout: STATUS_POLL_MS * 2 },
    );
  }, 20_000);

  it("does not add an Android floating status affordance while healthy", async () => {
    withClient(<StatusChip attentionOnly />);
    await waitFor(() => expect(screen.queryByRole("status")).toBeNull());
  });
});
