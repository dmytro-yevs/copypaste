import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { CloudCredentials, CloudStatusData } from "@/lib/ipc";
import { withUser } from "@/test/harness";
import { CloudSyncSettings } from "./CloudSyncSettings";

const ipc = vi.hoisted(() => ({
  cloudSetEndpoint: vi.fn(),
  cloudSignUp: vi.fn(),
  getCloudStatus: vi.fn(),
}));

vi.mock("@/lib/ipc", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/ipc")>()),
  cloudSetEndpoint: (url: string, anonKey: string) => ipc.cloudSetEndpoint(url, anonKey),
  cloudSignUp: (credentials: CloudCredentials) => ipc.cloudSignUp(credentials),
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
  ipc.cloudSignUp.mockReset().mockResolvedValue(status({
    configured: true,
    signed_in: true,
    key_ready: true,
    email: "person@example.com",
  }));
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
