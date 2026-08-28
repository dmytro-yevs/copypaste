import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { CloudCredentials, CloudStatusData } from "@/lib/ipc";
import { withUser } from "@/test/harness";
import { CloudSyncSettings } from "./CloudSyncSettings";
import styles from "./CloudSyncSettings.module.css";

const cloudStyles = readFileSync(
  resolve(process.cwd(), "src/features/settings/patterns/CloudSyncSettings.module.css"),
  "utf8",
);

const ipc = vi.hoisted(() => ({
  cloudSetEndpoint: vi.fn(),
  cloudSignOut: vi.fn(),
  cloudSignUp: vi.fn(),
  syncCloudNow: vi.fn(),
  getCloudStatus: vi.fn(),
}));

vi.mock("@/lib/ipc", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/ipc")>()),
  cloudSetEndpoint: (url: string, anonKey: string) => ipc.cloudSetEndpoint(url, anonKey),
  cloudSignOut: () => ipc.cloudSignOut(),
  cloudSignUp: (credentials: CloudCredentials) => ipc.cloudSignUp(credentials),
  syncCloudNow: () => ipc.syncCloudNow(),
  getCloudStatus: () => ipc.getCloudStatus(),
}));

function status(over: Partial<CloudStatusData> = {}): CloudStatusData {
  return {
    configured: false,
    signed_in: false,
    key_ready: false,
    email: null,
    last_sync_ms: null,
    last_error: null,
    poll_interval_secs: 60,
    unreadable_uploads: 0,
    ...over,
  };
}

beforeEach(() => {
  ipc.getCloudStatus.mockReset().mockResolvedValue(status());
  ipc.cloudSetEndpoint.mockReset().mockResolvedValue(status({ configured: true }));
  ipc.cloudSignOut.mockReset().mockResolvedValue(status({ configured: true }));
  ipc.cloudSignUp.mockReset().mockResolvedValue(status({
    configured: true,
    signed_in: true,
    key_ready: true,
    email: "person@example.com",
  }));
  ipc.syncCloudNow.mockReset().mockResolvedValue({
    uploaded: 0,
    tombstoned: 0,
    downloaded: 0,
    applied: 0,
    skipped_sensitive: 0,
    skipped_undecryptable: 0,
    skipped_forged: 0,
    skipped_future: 0,
    skipped_too_large: 0,
  });
});

describe("cloud account lifecycle", () => {
  it("lets an unconfigured build save a cloud endpoint", async () => {
    const { user } = withUser(<CloudSyncSettings />);

    await user.type(await screen.findByLabelText("Server URL"), "https://cloud.example.test");
    await user.type(screen.getByLabelText("Publishable key"), "publishable-anon-key");
    await user.click(screen.getByRole("button", { name: "Configure" }));

    await waitFor(() => expect(ipc.cloudSetEndpoint).toHaveBeenCalledWith(
      "https://cloud.example.test",
      "publishable-anon-key",
    ));
    expect((await screen.findByRole("button", { name: "Change server" }) as HTMLButtonElement).disabled)
      .toBe(false);
  });

  it("makes account creation reachable from the configured signed-out state", async () => {
    ipc.getCloudStatus.mockResolvedValue(status({ configured: true }));
    const { user } = withUser(<CloudSyncSettings />);

    await user.type(await screen.findByLabelText("Email"), "person@example.com");
    await user.type(screen.getByLabelText("Password"), "account-password");
    await user.type(screen.getByLabelText("Sync passphrase"), "a long sync passphrase");
    await user.click(screen.getByRole("button", { name: "Create account" }));

    await waitFor(() => expect(ipc.cloudSignUp).toHaveBeenCalledWith({
      email: "person@example.com",
      password: "account-password",
      passphrase: "a long sync passphrase",
    }));
    expect((await screen.findByText("person@example.com")).textContent)
      .toBe("person@example.com");
  });
});

describe("connection error announcements", () => {
  function expectSingleConnectionAlert(message: string | null) {
    const alerts = screen.getAllByRole("alert");
    expect(alerts).toHaveLength(1);
    const [alert] = alerts;
    expect(alert.classList.contains(styles.connectionNote)).toBe(true);
    expect(alert.getAttribute("aria-live")).toBe("assertive");
    expect(alert.getAttribute("aria-atomic")).toBe("true");
    expect(alert.querySelector('[role="alert"]')).toBeNull();
    if (message === null) {
      expect(alert.childNodes).toHaveLength(0);
      expect(alert.textContent).toBe("");
    } else {
      expect(alert.textContent).toContain(message);
    }
    return alert;
  }

  it("keeps the empty owner in flow without an inline line box", () => {
    const emptyRule = cloudStyles.match(
      /\.setupCopy > \.connectionNote:empty\s*\{([^}]*)\}/,
    );
    const populatedRule = cloudStyles.match(/\.connectionNote\s*\{([^}]*)\}/);
    expect(emptyRule?.[1]).toMatch(/display:\s*block/);
    expect(emptyRule?.[1]).toMatch(/block-size:\s*0/);
    expect(emptyRule?.[1]).toMatch(/margin-block-start:\s*0/);
    expect(populatedRule?.[1]).toMatch(/display:\s*inline-flex/);
  });

  it("keeps one stable atomic owner through failures, success, and a second failure", async () => {
    ipc.getCloudStatus.mockResolvedValue(status({
      configured: true,
      signed_in: true,
      key_ready: true,
      email: "person@example.com",
    }));
    ipc.syncCloudNow
      .mockRejectedValueOnce(new Error("sync failed"))
      .mockResolvedValueOnce({
        uploaded: 0,
        tombstoned: 0,
        downloaded: 0,
        applied: 0,
        skipped_sensitive: 0,
        skipped_undecryptable: 0,
        skipped_forged: 0,
        skipped_future: 0,
        skipped_too_large: 0,
      });
    ipc.cloudSignOut.mockRejectedValueOnce(new Error("sign-out failed"));
    const { user } = withUser(<CloudSyncSettings />);

    const initialOwner = await screen.findByRole("alert");
    expectSingleConnectionAlert(null);

    await screen.findByRole("button", { name: "Sync cloud now" });
    await user.click(screen.getByRole("button", { name: "Sync cloud now" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain(
      "Cloud sync failed. Check the connection and try again.",
    ));
    expect(screen.getByRole("alert")).toBe(initialOwner);
    expectSingleConnectionAlert("Cloud sync failed. Check the connection and try again.");

    await user.click(screen.getByRole("button", { name: "Sync cloud now" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toBe(""));
    expect(screen.getByRole("alert")).toBe(initialOwner);
    expectSingleConnectionAlert(null);

    await user.click(screen.getByRole("button", { name: "Sign out" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain(
      "Cloud sign-out failed. Try again.",
    ));
    expect(screen.getByRole("alert")).toBe(initialOwner);
    expectSingleConnectionAlert("Cloud sign-out failed. Try again.");
  });

  it("preserves persisted sync error priority and copy", async () => {
    ipc.getCloudStatus.mockResolvedValue(status({
      configured: true,
      last_error: "safe persisted error",
      unreadable_uploads: 2,
    }));
    withUser(<CloudSyncSettings />);

    await screen.findByText("The last cloud sync failed. Try again or sign in again.");
    expectSingleConnectionAlert("The last cloud sync failed. Try again or sign in again.");
  });

  it("announces unreadable uploads when no persisted sync error exists", async () => {
    ipc.getCloudStatus.mockResolvedValue(status({
      configured: true,
      unreadable_uploads: 2,
    }));
    withUser(<CloudSyncSettings />);

    await screen.findByText(
      "2 items on this device could not be prepared for cloud sync and are being retried.",
    );
    expectSingleConnectionAlert(
      "2 items on this device could not be prepared for cloud sync and are being retried.",
    );
  });
});
