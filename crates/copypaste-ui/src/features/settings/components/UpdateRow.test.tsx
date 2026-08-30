import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { UpdateStatus } from "@/lib/updater";
import { withUser } from "@/test/harness";
import { UpdateRow } from "./UpdateRow";

const updater = vi.hoisted(() => ({
  checkForUpdate: vi.fn(),
  getUpdateStatus: vi.fn(),
  installUpdate: vi.fn(),
}));

const platform = vi.hoisted(() => ({
  currentPlatform: vi.fn(() => "windows"),
}));

vi.mock("@/lib/updater", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/updater")>()),
  checkForUpdate: () => updater.checkForUpdate(),
  getUpdateStatus: () => updater.getUpdateStatus(),
  installUpdate: (...args: Parameters<typeof updater.installUpdate>) =>
    updater.installUpdate(...args),
}));

vi.mock("@/lib/platform", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/platform")>()),
  currentPlatform: () => platform.currentPlatform(),
}));

beforeEach(() => {
  updater.checkForUpdate.mockReset().mockResolvedValue({ state: "ready" });
  updater.getUpdateStatus.mockReset().mockResolvedValue({ state: "ready" });
  updater.installUpdate.mockReset().mockResolvedValue({ state: "ready" });
  platform.currentPlatform.mockReset().mockReturnValue("windows");
});

function renderStatus(status: UpdateStatus) {
  updater.getUpdateStatus.mockResolvedValue(status);
  return withUser(<UpdateRow />);
}

describe("UpdateRow static states", () => {
  it("keeps unsupported update support visible and non-actionable", async () => {
    const { container } = renderStatus({ state: "unsupported" });

    expect(await screen.findByText("Native CopyPaste app required.")).toBeTruthy();
    expect(screen.getByText("Install the native app to check and install updates here.")).toBeTruthy();
    expect(screen.getByText("Unavailable")).toBeTruthy();
    expect(screen.getByRole("status").getAttribute("aria-live")).toBe("polite");
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(container.querySelector(".activity")).toBeNull();
    expect(updater.checkForUpdate).not.toHaveBeenCalled();
    expect(updater.installUpdate).not.toHaveBeenCalled();
  });

  it("keeps unconfigured update support visible and non-actionable", async () => {
    const { container } = renderStatus({ state: "unconfigured" });

    expect(await screen.findByText("Updates aren't configured in this build.")).toBeTruthy();
    expect(screen.getByText("Checks the signed CopyPaste release feed for Windows.")).toBeTruthy();
    expect(screen.getByText("Not configured")).toBeTruthy();
    expect(screen.getByRole("status").getAttribute("aria-live")).toBe("polite");
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(container.querySelector(".activity")).toBeNull();
    expect(updater.checkForUpdate).not.toHaveBeenCalled();
    expect(updater.installUpdate).not.toHaveBeenCalled();
  });
});

describe("UpdateRow actions", () => {
  it("retains the check action for a ready updater", async () => {
    updater.checkForUpdate.mockResolvedValue({ state: "up_to_date" });
    const { user } = renderStatus({ state: "ready" });

    const check = await screen.findByRole("button", { name: "Check for updates" });
    await user.click(check);

    await waitFor(() => expect(updater.checkForUpdate).toHaveBeenCalledOnce());
    expect(await screen.findByText("CopyPaste is up to date.")).toBeTruthy();
  });
});
