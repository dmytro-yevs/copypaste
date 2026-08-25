import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { TooltipProvider } from "@/components/ui";
import type { ConfigApplied, ConfigData, ConfigPatch } from "@/lib/ipc";
import { withUser } from "@/test/harness";
import {
  AdvancedServiceSettings,
  PrivacyServiceSettings,
} from "./ServiceTab";

const ipc = vi.hoisted(() => ({
  getConfig: vi.fn(),
  getPrivateMode: vi.fn(),
  restartService: vi.fn(),
  setConfig: vi.fn(),
  setPrivateMode: vi.fn(),
}));

vi.mock("@/lib/ipc", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/ipc")>()),
  getConfig: () => ipc.getConfig(),
  getPrivateMode: () => ipc.getPrivateMode(),
  restartService: () => ipc.restartService(),
  setConfig: (patch: ConfigPatch) => ipc.setConfig(patch),
  setPrivateMode: (enabled: boolean) => ipc.setPrivateMode(enabled),
}));

function config(over: Partial<ConfigData> = {}): ConfigData {
  return {
    private_mode: false,
    poll_interval_ms: 500,
    history_limit: 10_000,
    storage_quota_bytes: 10 * 1_073_741_824,
    retention_days: 0,
    dedup_window_secs: 30,
    max_text_size_bytes: 4 * 1_048_576,
    max_image_size_bytes: 4 * 1_048_576,
    max_file_size_bytes: 4 * 1_048_576,
    max_decoded_image_mb: 50,
    sensitive_ttl_secs: 30,
    excluded_app_bundle_ids: [],
    lan_visibility: true,
    sync_enabled: true,
    notify_on_copy: false,
    sound_on_copy: false,
    ...over,
  };
}

function applied(
  over: Partial<ConfigData> = {},
  restartRequired: string[] = [],
): ConfigApplied {
  return { config: config(over), restart_required: restartRequired };
}

beforeEach(() => {
  ipc.getConfig.mockReset().mockResolvedValue(applied());
  ipc.getPrivateMode.mockReset().mockResolvedValue({
    private_mode: false,
    private_mode_epoch: 0,
  });
  ipc.setConfig.mockReset().mockImplementation((patch: ConfigPatch) =>
    Promise.resolve(
      applied(
        patch as Partial<ConfigData>,
        Object.keys(patch),
      ),
    ),
  );
  ipc.setPrivateMode.mockReset().mockResolvedValue({
    private_mode: true,
    private_mode_epoch: 1,
  });
  ipc.restartService.mockReset().mockResolvedValue({
    state: "running",
    version: "2.0.0-alpha.32",
    matches_app: true,
    ours: true,
  });
});

describe("service setting ownership", () => {
  it("does not inject a service error row beneath valid device-sync content", async () => {
    ipc.getConfig.mockRejectedValue({ code: "offline", retryable: true });

    withUser(
      <TooltipProvider>
        <p>Device sync remains available.</p>
        <AdvancedServiceSettings />
      </TooltipProvider>,
    );

    expect(screen.getByText("Device sync remains available.")).toBeTruthy();
    await waitFor(() => expect(ipc.getConfig).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(
      screen.queryByText("The clipboard service is not running."),
    ).toBeNull();
    expect(
      screen.queryByRole("switch", { name: "Visible on this network" }),
    ).toBeNull();
  });

  it("saves one advanced field and offers the service-requested restart", async () => {
    const { user } = withUser(
      <TooltipProvider>
        <AdvancedServiceSettings />
      </TooltipProvider>,
    );
    const visibility = await screen.findByRole("switch", {
      name: "Visible on this network",
    });

    expect(ipc.getPrivateMode).not.toHaveBeenCalled();
    await user.click(visibility);

    await waitFor(() =>
      expect(ipc.setConfig).toHaveBeenCalledWith({ lan_visibility: false }),
    );
    expect(
      screen
        .getByText("Saved. It takes effect after the service restarts.")
        .closest('[role="status"]'),
    ).toBeTruthy();
    await user.click(
      await screen.findByRole("button", { name: "Restart the service" }),
    );
    await waitFor(() => expect(ipc.restartService).toHaveBeenCalledTimes(1));
  });

  it("keeps private mode on its dedicated service mutation", async () => {
    const { user } = withUser(
      <TooltipProvider>
        <PrivacyServiceSettings />
      </TooltipProvider>,
    );
    await user.click(
      await screen.findByRole("switch", { name: "Private mode" }),
    );

    await waitFor(() => expect(ipc.setPrivateMode).toHaveBeenCalledWith(true));
    expect(ipc.setConfig).not.toHaveBeenCalled();
  });
});
