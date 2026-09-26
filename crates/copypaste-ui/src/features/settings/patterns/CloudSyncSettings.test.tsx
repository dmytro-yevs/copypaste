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

    const setup = await screen.findByRole("button", { name: "Set up cloud sync" });
    const advanced = screen.getByText("Advanced · Self-hosted cloud server").closest("details");
    expect(advanced?.open).toBe(false);
    expect(screen.getByRole("textbox", { name: "Server URL" }).closest("details")?.open).toBe(false);
    expect(screen.getByText(/Direct device sync keeps working without one/)).toBeTruthy();
    await user.click(setup);
    await waitFor(() => expect(advanced?.open).toBe(true));

    await user.type(screen.getByRole("textbox", { name: "Server URL" }), "https://cloud.example.test");
    await user.type(screen.getByLabelText("Publishable key"), "publishable-anon-key");
    await user.click(screen.getByRole("button", { name: "Configure" }));

    await waitFor(() => expect(ipc.cloudSetEndpoint).toHaveBeenCalledWith(
      "https://cloud.example.test",
      "publishable-anon-key",
    ));
    expect((await screen.findByRole("button", { name: "Change server" }) as HTMLButtonElement).disabled)
      .toBe(false);
  });

  it("lets the setup button reopen a disclosure previously opened by search", async () => {
    const { user, rerender } = withUser(
      <CloudSyncSettings revealAdvancedKey="settings.sync.cloud.endpoint.url:1" />,
    );
    const summary = await screen.findByText("Advanced · Self-hosted cloud server");
    const advanced = summary.closest("details");
    await waitFor(() => expect(advanced?.open).toBe(true));
    await user.click(summary);
    expect(advanced?.open).toBe(false);

    await user.click(screen.getByRole("button", { name: "Set up cloud sync" }));
    await waitFor(() => expect(advanced?.open).toBe(true));
    await user.click(summary);
    rerender(<CloudSyncSettings revealAdvancedKey="settings.sync.cloud.endpoint.url:2" />);
    await waitFor(() => expect(advanced?.open).toBe(true));
  });

  it("keeps account sign-in primary and preserves advanced server drafts and restore", async () => {
    ipc.getCloudStatus.mockResolvedValue(status({ configured: true }));
    ipc.cloudSetEndpoint.mockImplementation((url: string) => Promise.resolve(status({
      configured: Boolean(url),
    })));
    const { user } = withUser(<CloudSyncSettings />);

    expect(await screen.findByLabelText("Email")).toBeTruthy();
    const summary = screen.getByText("Advanced · Self-hosted cloud server");
    const advanced = summary.closest("details");
    expect(advanced?.open).toBe(false);
    expect(screen.getByRole("button", { name: "Change server" }).closest("details")?.open).toBe(false);

    await user.click(summary);
    await user.click(screen.getByRole("button", { name: "Change server" }));
    expect(screen.getByText(/Changing or restoring the cloud server signs this device out/)).toBeTruthy();
    const url = screen.getByRole("textbox", { name: "Server URL" }) as HTMLInputElement;
    const key = screen.getByRole("textbox", { name: "Publishable key" }) as HTMLInputElement;
    expect(url.value).toBe("");
    expect(key.value).toBe("");
    await user.type(url, "https://other.example.test");
    await user.type(key, "other-anon-key");
    await user.click(summary);
    expect(advanced?.open).toBe(false);
    expect(advanced?.querySelector("summary")?.textContent).toContain("Unsaved server credentials");
    await user.click(summary);
    expect(url.value).toBe("https://other.example.test");
    expect(key.value).toBe("other-anon-key");
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("textbox", { name: "Server URL" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Change server" }));
    await user.type(screen.getByRole("textbox", { name: "Server URL" }), "https://replacement.example.test");
    await user.type(screen.getByRole("textbox", { name: "Publishable key" }), "replacement-anon-key");
    await user.click(screen.getByRole("button", { name: "Save server" }));
    await waitFor(() => expect(ipc.cloudSetEndpoint).toHaveBeenCalledWith(
      "https://replacement.example.test",
      "replacement-anon-key",
    ));
    await user.click(await screen.findByRole("button", { name: "Change server" }));
    await user.click(screen.getByRole("button", { name: "Restore hosted default" }));
    await waitFor(() => expect(ipc.cloudSetEndpoint).toHaveBeenCalledWith("", ""));
    expect(await screen.findByRole("button", { name: "Set up cloud sync" })).toBeTruthy();
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

describe("cloud connection health", () => {
  it.each([
    ["last sync error", { last_error: "safe persisted error" }],
    ["unreadable uploads", { unreadable_uploads: 2 }],
  ] as const)("shows attention for %s while keeping account actions", async (_case, issue) => {
    ipc.getCloudStatus.mockResolvedValue(status({
      configured: true,
      signed_in: true,
      key_ready: true,
      email: "person@example.com",
      ...issue,
    }));
    withUser(<CloudSyncSettings />);

    expect(await screen.findByText("Needs attention")).toBeTruthy();
    expect(screen.getByText("Your account is connected, but cloud sync needs attention.")).toBeTruthy();
    expect(screen.queryByText("Connected")).toBeNull();
    expect(screen.getByRole("button", { name: "Sync cloud now" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sign out" })).toBeTruthy();
    expect(screen.getByText("Advanced · Self-hosted cloud server").closest("details")?.open).toBe(false);
    expect(screen.queryByLabelText("Password")).toBeNull();
  });

  it("shows healthy only when the signed-in account has no sync issue", async () => {
    ipc.getCloudStatus.mockResolvedValue(status({
      configured: true,
      signed_in: true,
      key_ready: true,
    }));
    withUser(<CloudSyncSettings />);

    expect(await screen.findByText("Connected")).toBeTruthy();
    expect(screen.queryByText("Needs attention")).toBeNull();
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
    expect(screen.getByText("Needs attention")).toBeTruthy();
    expect(screen.queryByText("Connected")).toBeNull();
    expect(screen.getByRole("alert")).toBe(initialOwner);
    expectSingleConnectionAlert("Cloud sync failed. Check the connection and try again.");

    await user.click(screen.getByRole("button", { name: "Sync cloud now" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toBe(""));
    expect(screen.getByText("Connected")).toBeTruthy();
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
