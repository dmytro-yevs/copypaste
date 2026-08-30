import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { CloudCredentials, CloudStatusData } from "@/lib/ipc";
import { withUser } from "@/test/harness";
import { CloudSyncSettings } from "./CloudSyncSettings";
import styles from "./CloudSyncSettings.module.css";

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
  function expectSingleAlert(message: string) {
    const alerts = screen.getAllByRole("alert");
    expect(alerts).toHaveLength(1);
    const [alert] = alerts;
    expect(alert.getAttribute("aria-live")).toBe("assertive");
    expect(alert.parentElement?.classList.contains(styles.connectionNote)).toBe(true);
    expect(alert.parentElement?.tagName).toBe("SPAN");
    expect(alert.textContent).toContain(message);
  }

  it("keeps a sync failure as one assertive connection alert", async () => {
    ipc.getCloudStatus.mockResolvedValue(status({
      configured: true,
      signed_in: true,
      key_ready: true,
      email: "person@example.com",
    }));
    ipc.syncCloudNow.mockRejectedValue(new Error("sync failed"));
    const { user } = withUser(<CloudSyncSettings />);

    await user.click(await screen.findByRole("button", { name: "Sync cloud now" }));
    await screen.findByRole("alert");

    expectSingleAlert("Cloud sync failed. Check the connection and try again.");
  });

  it("keeps a sign-out failure as one assertive connection alert", async () => {
    ipc.getCloudStatus.mockResolvedValue(status({
      configured: true,
      signed_in: true,
      key_ready: true,
      email: "person@example.com",
    }));
    ipc.cloudSignOut.mockRejectedValue(new Error("sign-out failed"));
    const { user } = withUser(<CloudSyncSettings />);

    await user.click(await screen.findByRole("button", { name: "Sign out" }));
    await screen.findByRole("alert");

    expectSingleAlert("Cloud sign-out failed. Try again.");
  });

  it("keeps the persisted sync failure as one assertive connection alert", async () => {
    ipc.getCloudStatus.mockResolvedValue(status({
      configured: true,
      last_error: "safe persisted error",
    }));
    withUser(<CloudSyncSettings />);

    await screen.findByRole("alert");

    expectSingleAlert("The last cloud sync failed. Try again or sign in again.");
  });

  it("keeps unreadable upload feedback as one assertive connection alert", async () => {
    ipc.getCloudStatus.mockResolvedValue(status({
      configured: true,
      unreadable_uploads: 2,
    }));
    withUser(<CloudSyncSettings />);

    await screen.findByRole("alert");

    expectSingleAlert(
      "2 items on this device could not be prepared for cloud sync and are being retried.",
    );
  });
});
