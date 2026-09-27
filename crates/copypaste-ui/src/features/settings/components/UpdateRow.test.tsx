import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { UpdateStatus } from "@/lib/updater";
import { PRODUCT_RELEASES_URL } from "@/lib/productLinks";
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((onResolve) => {
    resolve = onResolve;
  });
  return { promise, resolve };
}

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
  it("offers official releases when macOS is not managed by Homebrew", async () => {
    platform.currentPlatform.mockReturnValue("macos");
    renderStatus({ state: "unconfigured" });

    expect(await screen.findByText("Automatic updates require the CopyPaste Homebrew cask.")).toBeTruthy();
    expect(screen.getByText("For this installation, get updates from the official releases page.")).toBeTruthy();
    const releases = screen.getByRole("link", { name: "View releases" });
    expect(releases.getAttribute("href")).toBe(PRODUCT_RELEASES_URL);
    expect(releases.getAttribute("target")).toBe("_blank");
    expect(releases.getAttribute("rel")).toContain("noreferrer");
    expect(updater.checkForUpdate).not.toHaveBeenCalled();
    expect(updater.installUpdate).not.toHaveBeenCalled();
  });

  it("keeps unsupported update support visible and non-actionable", async () => {
    renderStatus({ state: "unsupported" });

    expect(await screen.findByText("Native CopyPaste app required.")).toBeTruthy();
    expect(screen.getByText("Install the native app to check and install updates here.")).toBeTruthy();
    expect(screen.getByText("Unavailable")).toBeTruthy();
    expect(screen.getByRole("status").getAttribute("aria-live")).toBe("polite");
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(updater.checkForUpdate).not.toHaveBeenCalled();
    expect(updater.installUpdate).not.toHaveBeenCalled();
  });

  it("keeps unconfigured update support visible and non-actionable", async () => {
    renderStatus({ state: "unconfigured" });

    expect(await screen.findByText("Updates aren't configured in this build.")).toBeTruthy();
    expect(screen.getByText("Checks the signed CopyPaste release feed for Windows.")).toBeTruthy();
    expect(screen.getByText("Not configured")).toBeTruthy();
    expect(screen.getByRole("status").getAttribute("aria-live")).toBe("polite");
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(updater.checkForUpdate).not.toHaveBeenCalled();
    expect(updater.installUpdate).not.toHaveBeenCalled();
  });
});

describe("UpdateRow actions", () => {
  it("shows a safe readiness error and recovers after retry", async () => {
    platform.currentPlatform.mockReturnValue("macos");
    updater.getUpdateStatus.mockRejectedValue({
      code: "update_check_failed",
      retryable: true,
      message: "Could not list /Users/private/Homebrew/Caskroom",
    });
    updater.checkForUpdate.mockResolvedValue({ state: "up_to_date" });
    const { user } = withUser(<UpdateRow />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("CopyPaste couldn't check for updates. Try again in a moment.");
    expect(document.body.textContent).not.toContain("/Users/private");

    await user.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(updater.checkForUpdate).toHaveBeenCalledOnce());
    expect(await screen.findByText("CopyPaste is up to date.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Check again" })).toBeTruthy();
    expect(document.body.textContent).not.toContain("/Users/private");
  });

  it("retains the check action for a ready updater", async () => {
    updater.checkForUpdate.mockResolvedValue({ state: "up_to_date" });
    const { user } = renderStatus({ state: "ready" });

    const check = await screen.findByRole("button", { name: "Check for updates" });
    await user.click(check);

    await waitFor(() => expect(updater.checkForUpdate).toHaveBeenCalledOnce());
    expect(await screen.findByText("CopyPaste is up to date.")).toBeTruthy();
  });

  it("keeps the check action disabled and busy while the check is pending", async () => {
    const pending = deferred<UpdateStatus>();
    updater.checkForUpdate.mockReturnValue(pending.promise);
    const { user } = renderStatus({ state: "ready" });

    await user.click(await screen.findByRole("button", { name: "Check for updates" }));

    const checking = screen.getByRole("button", { name: "Check for updates" });
    expect(checking.hasAttribute("disabled")).toBe(true);
    expect(checking.getAttribute("aria-busy")).toBe("true");
    expect(screen.getByRole("status").textContent).toBe("Checking for updates…");

    pending.resolve({ state: "up_to_date" });
    expect(await screen.findByText("CopyPaste is up to date.")).toBeTruthy();
  });

  it("keeps the compact check row while a failed check recovers", async () => {
    const pending = deferred<UpdateStatus>();
    updater.checkForUpdate
      .mockRejectedValueOnce({ code: "update_check_failed", retryable: true })
      .mockReturnValueOnce(pending.promise);
    const { user } = renderStatus({ state: "ready" });
    const row = (await screen.findByText("App updates")).closest("section");
    expect(row).not.toBeNull();

    await user.click(screen.getByRole("button", { name: "Check for updates" }));

    expect((await screen.findByRole("alert")).textContent).toBe(
      "CopyPaste couldn't check for updates. Try again in a moment.",
    );
    expect(row?.getAttribute("data-state")).toBe("error");

    await user.click(screen.getByRole("button", { name: "Try again" }));

    const checking = screen.getByRole("button", { name: "Check for updates" });
    expect(checking.hasAttribute("disabled")).toBe(true);
    expect(checking.getAttribute("aria-busy")).toBe("true");
    expect(row?.getAttribute("data-state")).toBe("checking");

    pending.resolve({ state: "up_to_date" });
    expect(await screen.findByText("CopyPaste is up to date.")).toBeTruthy();
    expect(row?.getAttribute("data-state")).toBe("up_to_date");
  });
});
