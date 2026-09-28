import { act, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { UpdateProgress, UpdateStatus } from "@/lib/updater";
import { PRODUCT_RELEASES_URL } from "@/lib/productLinks";
import { TooltipProvider } from "@/components/ui/tooltip";
import { withUser } from "@/test/harness";
import { SettingsSchemaField } from "./SettingsSchemaRenderer";
import { useUpdateSetting } from "./useUpdateSetting";

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
  return renderSetting();
}

function UpdateSettingHarness() {
  const { field, dialog } = useUpdateSetting();
  return <><SettingsSchemaField field={field} />{dialog}</>;
}

function renderSetting() {
  return withUser(<TooltipProvider><UpdateSettingHarness /></TooltipProvider>);
}

describe("update setting static states", () => {
  it("offers official releases when macOS is not managed by Homebrew", async () => {
    platform.currentPlatform.mockReturnValue("macos");
    const { user } = renderStatus({ state: "unconfigured" });

    expect(await screen.findByText("Automatic updates require the CopyPaste Homebrew cask.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "More about App updates" }));
    expect(await screen.findByText("For this installation, get updates from the official releases page.")).toBeTruthy();
    const releases = screen.getByRole("link", { name: "View releases" });
    expect(releases.getAttribute("href")).toBe(PRODUCT_RELEASES_URL);
    expect(releases.getAttribute("target")).toBe("_blank");
    expect(releases.getAttribute("rel")).toContain("noreferrer");
    expect(updater.checkForUpdate).not.toHaveBeenCalled();
    expect(updater.installUpdate).not.toHaveBeenCalled();
  });

  it("keeps unsupported update support visible and non-actionable", async () => {
    const { user } = renderStatus({ state: "unsupported" });

    expect(await screen.findByText("Native CopyPaste app required.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "More about App updates" }));
    expect(await screen.findByText("Install the native app to check and install updates here.")).toBeTruthy();
    expect(screen.getByText("Unavailable")).toBeTruthy();
    expect(screen.getByRole("status", { name: "Unavailable" }).getAttribute("aria-live")).toBe("polite");
    expect(screen.queryByRole("button", { name: "Check for updates" })).toBeNull();
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(updater.checkForUpdate).not.toHaveBeenCalled();
    expect(updater.installUpdate).not.toHaveBeenCalled();
  });

  it("keeps unconfigured update support visible and non-actionable", async () => {
    const { user } = renderStatus({ state: "unconfigured" });

    expect(await screen.findByText("Updates aren't configured in this build.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "More about App updates" }));
    expect(await screen.findByText("Checks the signed CopyPaste release feed for Windows.")).toBeTruthy();
    expect(screen.getByText("Not configured")).toBeTruthy();
    expect(screen.getByRole("status", { name: "Not configured" }).getAttribute("aria-live")).toBe("polite");
    expect(screen.queryByRole("button", { name: "Check for updates" })).toBeNull();
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(updater.checkForUpdate).not.toHaveBeenCalled();
    expect(updater.installUpdate).not.toHaveBeenCalled();
  });
});

describe("update setting actions", () => {
  it("shows a safe readiness error and recovers after retry", async () => {
    platform.currentPlatform.mockReturnValue("macos");
    updater.getUpdateStatus.mockRejectedValue({
      code: "update_check_failed",
      retryable: true,
      message: "Could not list /Users/private/Homebrew/Caskroom",
    });
    updater.checkForUpdate.mockResolvedValue({ state: "up_to_date" });
    const { user } = renderSetting();

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
    const row = (await screen.findByText("App updates")).closest('[data-settings-search-target="row:App updates"]');
    expect(row).not.toBeNull();

    await user.click(screen.getByRole("button", { name: "Check for updates" }));

    expect((await screen.findByRole("alert")).textContent).toBe(
      "CopyPaste couldn't check for updates. Try again in a moment.",
    );
    expect(row?.querySelector('[role="alert"]')).not.toBeNull();

    await user.click(screen.getByRole("button", { name: "Try again" }));

    const checking = screen.getByRole("button", { name: "Check for updates" });
    expect(checking.hasAttribute("disabled")).toBe(true);
    expect(checking.getAttribute("aria-busy")).toBe("true");
    expect(row?.querySelector('[data-mode="loading"]')).not.toBeNull();

    pending.resolve({ state: "up_to_date" });
    expect(await screen.findByText("CopyPaste is up to date.")).toBeTruthy();
    expect(row?.querySelector('[data-mode="loading"]')).toBeNull();
  });

  it("confirms a Windows update and reports download and verification progress", async () => {
    const pending = deferred<UpdateStatus>();
    updater.installUpdate.mockReturnValue(pending.promise);
    const { user } = renderStatus({ state: "available", version: "2.0.0" });

    await user.click(await screen.findByRole("button", { name: "Install update" }));
    expect(screen.getByRole("alertdialog", { name: "Install CopyPaste 2.0.0?" })).toBeTruthy();
    expect(screen.getByText("CopyPaste will download and verify the signed Windows release, install it, then restart.")).toBeTruthy();
    expect(updater.installUpdate).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Install and restart" }));
    expect(updater.installUpdate).toHaveBeenCalledOnce();
    expect(updater.installUpdate.mock.calls[0]?.[0]).toBe("2.0.0");
    expect(screen.queryByRole("alertdialog")).toBeNull();

    const onProgress = updater.installUpdate.mock.calls[0]?.[1] as (progress: UpdateProgress) => void;
    act(() => onProgress({ state: "downloading", downloaded: 25, total: 100 }));
    const progress = screen.getByRole("progressbar", { name: "Downloading CopyPaste 2.0.0 update" });
    expect(progress.getAttribute("value")).toBe("25");
    expect(screen.getByText("Downloading the update… 25%").closest('[role="status"]')?.getAttribute("aria-live")).toBe("polite");

    act(() => onProgress({ state: "verifying" }));
    expect(screen.getByText("Download complete. Verifying the update…")).toBeTruthy();
    pending.resolve({ state: "ready" });
    expect(await screen.findByRole("button", { name: "Check for updates" })).toBeTruthy();
  });

  it("lets a macOS user defer the install and explicitly confirm it later", async () => {
    platform.currentPlatform.mockReturnValue("macos");
    const { user } = renderStatus({ state: "available", version: "2.0.0" });

    await user.click(await screen.findByRole("button", { name: "Install update" }));
    expect(screen.getByText("Homebrew will upgrade the installed cask, then CopyPaste will restart.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Later" }));
    expect(await screen.findByText("CopyPaste 2.0.0 will wait until you choose to install it.")).toBeTruthy();
    expect(updater.installUpdate).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Install update" }));
    await user.click(screen.getByRole("button", { name: "Update and restart" }));
    await waitFor(() => expect(updater.installUpdate).toHaveBeenCalledOnce());
    expect(updater.installUpdate.mock.calls[0]?.[0]).toBe("2.0.0");
  });

  it("continues an Android update after an installer permission error", async () => {
    platform.currentPlatform.mockReturnValue("android");
    updater.installUpdate
      .mockRejectedValueOnce({ code: "update_permission_required", retryable: true })
      .mockResolvedValueOnce({ state: "ready" });
    const { user } = renderStatus({ state: "available", version: "2.0.0" });

    await user.click(await screen.findByRole("button", { name: "Install update" }));
    expect(screen.getByText("CopyPaste will download and verify the signed APK, ask Android to install it, then reopen.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Install and restart" }));

    const continueButton = await screen.findByRole("button", { name: "Continue update" });
    await user.click(continueButton);
    await waitFor(() => expect(updater.installUpdate).toHaveBeenCalledTimes(2));
    expect(updater.installUpdate.mock.calls[1]?.[0]).toBe("2.0.0");
  });

  it("retries a failed Windows installation for the same version", async () => {
    updater.installUpdate
      .mockRejectedValueOnce({ code: "update_install_failed", retryable: true, message: "private installer path" })
      .mockResolvedValueOnce({ state: "ready" });
    const { user } = renderStatus({ state: "available", version: "2.0.0" });

    await user.click(await screen.findByRole("button", { name: "Install update" }));
    await user.click(screen.getByRole("button", { name: "Install and restart" }));
    await user.click(await screen.findByRole("button", { name: "Try again" }));

    await waitFor(() => expect(updater.installUpdate).toHaveBeenCalledTimes(2));
    expect(updater.installUpdate.mock.calls[1]?.[0]).toBe("2.0.0");
    expect(updater.checkForUpdate).not.toHaveBeenCalled();
    expect(document.body.textContent).not.toContain("private installer path");
  });
});
