import { act, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { UpdateRow } from "@/components/settings/UpdateRow";
import type { UpdateProgress, UpdateStatus } from "@/lib/updater";
import { withClient, withUser } from "@/test/harness";

const getUpdateStatus = vi.fn();
const checkForUpdate = vi.fn();
const installUpdate = vi.fn();

vi.mock("@/lib/updater", () => ({
  getUpdateStatus: () => getUpdateStatus(),
  checkForUpdate: () => checkForUpdate(),
  installUpdate: (version: string, progress: (value: UpdateProgress) => void) =>
    installUpdate(version, progress),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

beforeEach(() => {
  getUpdateStatus.mockReset().mockResolvedValue({ state: "ready" });
  checkForUpdate.mockReset().mockResolvedValue({ state: "up_to_date" });
  installUpdate.mockReset();
});

describe("build support states", () => {
  it.each([
    ["unsupported", "In-app updates aren't available on this platform."],
    ["unconfigured", "Updates aren't configured in this build."],
  ])("names %s without offering an update action", async (state, message) => {
    getUpdateStatus.mockResolvedValue({ state });
    withClient(<UpdateRow />);

    expect((await screen.findByRole("status")).textContent).toContain(message);
    expect(screen.queryByRole("button")).toBeNull();
  });
});

it("announces checking and the up-to-date result", async () => {
  const pending = deferred<UpdateStatus>();
  checkForUpdate.mockReturnValue(pending.promise);
  const { user } = withUser(<UpdateRow />);

  await user.click(await screen.findByRole("button", { name: "Check for updates" }));
  expect(screen.getByRole("status").textContent).toContain("Checking for updates…");

  act(() => pending.resolve({ state: "up_to_date" }));
  expect(await screen.findByText("CopyPaste is up to date.")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Check again" })).toBeTruthy();
});

it("requires confirmation and records a user's refusal", async () => {
  checkForUpdate.mockResolvedValue({ state: "available", version: "2.0.0" });
  const { user } = withUser(<UpdateRow />);

  await user.click(await screen.findByRole("button", { name: "Check for updates" }));
  expect((await screen.findByRole("status")).textContent).toContain(
    "CopyPaste 2.0.0 is available.",
  );
  await user.click(screen.getByRole("button", { name: "Install update" }));
  expect(screen.getByRole("alertdialog", { name: "Install CopyPaste 2.0.0?" })).toBeTruthy();

  await user.click(screen.getByRole("button", { name: "Later" }));
  expect(screen.getByRole("status").textContent).toContain(
    "CopyPaste 2.0.0 will wait until you choose to install it.",
  );
  expect(installUpdate).not.toHaveBeenCalled();
});

it("names download, verification, and installer handoff progress", async () => {
  checkForUpdate.mockResolvedValue({ state: "available", version: "2.0.0" });
  const pending = deferred<UpdateStatus>();
  let progress!: (value: UpdateProgress) => void;
  installUpdate.mockImplementation((_version, callback) => {
    progress = callback;
    return pending.promise;
  });
  const { user } = withUser(<UpdateRow />);

  await user.click(await screen.findByRole("button", { name: "Check for updates" }));
  await user.click(await screen.findByRole("button", { name: "Install update" }));
  await user.click(screen.getByRole("button", { name: "Install and restart" }));

  act(() => progress({ state: "downloading", downloaded: 50, total: 100 }));
  const bar = screen.getByRole("progressbar", { name: "Downloading CopyPaste 2.0.0 update" });
  expect(bar.getAttribute("value")).toBe("50");
  expect(screen.getByRole("status").textContent).toContain("50%");

  act(() => progress({ state: "verifying" }));
  expect(screen.getByRole("status").textContent).toContain("Verifying the signed update");
  act(() => progress({ state: "installing" }));
  expect(screen.getByRole("status").textContent).toContain(
    "Opening the Windows installer",
  );
});

it("maps failures to actionable copy without rendering plugin details", async () => {
  checkForUpdate.mockRejectedValue({
    code: "update_network_failed",
    retryable: true,
    message: "C:\\Users\\alice\\private\\latest.json",
  });
  const { user, container } = withUser(<UpdateRow />);

  await user.click(await screen.findByRole("button", { name: "Check for updates" }));
  expect((await screen.findByRole("alert")).textContent).toContain(
    "CopyPaste couldn't reach the update service. Check your connection and try again.",
  );
  expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  await waitFor(() => expect(container.textContent).not.toContain("Users\\alice"));
});

it("refuses a bad signature without offering an automatic retry", async () => {
  checkForUpdate.mockRejectedValue({
    code: "update_signature_invalid",
    retryable: false,
  });
  const { user } = withUser(<UpdateRow />);

  await user.click(await screen.findByRole("button", { name: "Check for updates" }));
  expect((await screen.findByRole("alert")).textContent).toContain("could not be verified");
  expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
});
