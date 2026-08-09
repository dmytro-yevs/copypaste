/**
 * The service's settings, which had no interface at all until this tab.
 *
 * Two of these are the properties the daemon's own contract is built on, seen
 * from the screen that uses it: a value that round-trips, and a write that
 * carries only the field it names. They were previously exercised only by
 * `e2e/tests/daemon-config.e2e.test.ts` driving the CLI, because no Tauri
 * command routed `GetConfig` or `SetConfig`.
 *
 * The payload limits are independently live and patchable. The final contract
 * covered here is `sensitive_ttl_secs`: it ships off, and the reason recorded on
 * the field is that v2 had nowhere to say the sweep had happened — so the
 * assertions here are about what the control says, not about what it stores.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor } from "@testing-library/react";

import { ServiceTab } from "@/components/settings/ServiceTab";
import { IpcFailure } from "@/lib/errors";
import type { ConfigApplied, ConfigData, ConfigPatch } from "@/lib/ipc";
import { captureSnapshot, status, withUser } from "@/test/harness";

const getConfig = vi.fn();
const setConfig = vi.fn();
const getPrivateMode = vi.fn();
const setPrivateMode = vi.fn();
const getStatus = vi.fn();
const captureState = vi.fn();
const captureArm = vi.fn();
const captureRefresh = vi.fn();
const captureSetEnabled = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    getConfig: () => getConfig(),
    setConfig: (patch: ConfigPatch) => setConfig(patch),
    getPrivateMode: () => getPrivateMode(),
    setPrivateMode: (enabled: boolean) => setPrivateMode(enabled),
    getStatus: () => getStatus(),
    captureState: () => captureState(),
    captureArm: () => captureArm(),
    captureRefresh: () => captureRefresh(),
    captureSetEnabled: (enabled: boolean) => captureSetEnabled(enabled),
  };
});

function config(over: Partial<ConfigData> = {}): ConfigData {
  return {
    private_mode: false,
    poll_interval_ms: 500,
    history_limit: 10_000,
    storage_quota_bytes: 10 * 1_073_741_824,
    retention_days: 0,
    dedup_window_secs: 30,
    max_text_size_bytes: 10 * 1_048_576,
    max_image_size_bytes: 64 * 1_048_576,
    max_file_size_bytes: 100 * 1_048_576,
    max_decoded_image_mb: 50,
    sensitive_ttl_secs: 0,
    excluded_app_bundle_ids: [],
    lan_visibility: true,
    sync_enabled: true,
    notify_on_copy: false,
    sound_on_copy: false,
    ...over,
  };
}

const applied = (
  over: Partial<ConfigData> = {},
  restart: string[] = [],
): ConfigApplied => ({ config: config(over), restart_required: restart });

beforeEach(() => {
  getConfig.mockReset().mockResolvedValue(applied());
  setConfig.mockReset().mockImplementation(() => Promise.resolve(applied()));
  getPrivateMode.mockReset().mockResolvedValue({ private_mode: false });
  setPrivateMode
    .mockReset()
    .mockImplementation((enabled: boolean) => Promise.resolve({ private_mode: enabled }));
  getStatus.mockReset().mockResolvedValue(status());
  captureState.mockReset().mockResolvedValue(captureSnapshot());
  captureArm.mockReset().mockResolvedValue(captureSnapshot());
  captureRefresh.mockReset().mockResolvedValue(captureSnapshot());
  captureSetEnabled.mockReset().mockResolvedValue(captureSnapshot());
});

afterEach(() => vi.restoreAllMocks());

describe("reading the service's settings", () => {
  it("shows the live capture state in Service", async () => {
    withUser(<ServiceTab />);
    expect(await screen.findByText("Capturing from every app.")).toBeTruthy();
  });

  it("shows the value the service reported, not a guess", async () => {
    getConfig.mockResolvedValue(applied({ poll_interval_ms: 2000 }));
    withUser(<ServiceTab />);
    const poll = await screen.findByRole("combobox", {
      name: "Check the clipboard every",
    });
    expect((poll as HTMLSelectElement).value).toBe("2000");
  });

  /**
   * A value the CLI can set and this screen does not offer must appear as
   * itself. Snapping it to the nearest choice would make the screen disagree
   * with the service while looking like it agreed.
   */
  it("keeps a value it does not offer rather than snapping to a neighbour", async () => {
    getConfig.mockResolvedValue(applied({ poll_interval_ms: 1500 }));
    withUser(<ServiceTab />);
    const poll = await screen.findByRole("combobox", {
      name: "Check the clipboard every",
    });
    expect((poll as HTMLSelectElement).value).toBe("1500");
  });

  it("shows every binding payload default", async () => {
    withUser(<ServiceTab />);
    const expected = [
      ["Ignore text larger than", 10 * 1_048_576],
      ["Ignore images larger than", 64 * 1_048_576],
      ["Ignore files larger than", 100 * 1_048_576],
      ["Decoded image memory limit", 50],
    ] as const;
    for (const [name, value] of expected) {
      const control = await screen.findByRole("combobox", { name });
      expect((control as HTMLSelectElement).value).toBe(String(value));
    }
  });

  it("associates each payload control with concise visible help", async () => {
    withUser(<ServiceTab />);
    const file = await screen.findByRole("combobox", {
      name: "Ignore files larger than",
    });
    const helpId = file.getAttribute("aria-describedby");
    expect(helpId).toBeTruthy();
    expect(document.getElementById(helpId!)?.textContent).toContain(
      "effective limit is 4 MiB",
    );
  });

  it("shows a concise unavailable state when service settings cannot load", async () => {
    getConfig.mockRejectedValue(new IpcFailure("unavailable", false));
    withUser(<ServiceTab />);
    expect(await screen.findByText("Service settings are unavailable.")).toBeTruthy();
  });
});

describe("writing one", () => {
  it("can turn Android capture off from Service", async () => {
    const { user } = withUser(<ServiceTab />);
    await user.click(await screen.findByRole("switch", { name: "Capture from other apps" }));
    await waitFor(() => expect(captureSetEnabled).toHaveBeenCalledWith(false));
  });

  it("adds a validated source-app exclusion as its own persisted patch", async () => {
    const { user } = withUser(<ServiceTab />);
    const appId = await screen.findByRole("textbox", {
      name: "App bundle or package ID",
    });
    await user.type(appId, "com.example.private-app");
    await user.click(screen.getByRole("button", { name: "Add app" }));

    await waitFor(() =>
      expect(setConfig).toHaveBeenCalledWith({
        excluded_app_bundle_ids: ["com.example.private-app"],
      }),
    );
  });

  it("refuses a source-app exclusion that is not a bundle or package ID", async () => {
    const { user } = withUser(<ServiceTab />);
    const appId = await screen.findByRole("textbox", {
      name: "App bundle or package ID",
    });
    await user.type(appId, "not an app id");
    await user.click(screen.getByRole("button", { name: "Add app" }));

    expect((await screen.findByRole("alert")).textContent).toContain(
      "Enter a bundle or package ID",
    );
    expect(setConfig).not.toHaveBeenCalled();
  });

  it("sends a patch naming only the field that changed", async () => {
    const { user } = withUser(<ServiceTab />);
    const dedup = await screen.findByRole("combobox", {
      name: "Treat a repeat as the same item for",
    });
    await user.selectOptions(dedup, "60");

    await waitFor(() => expect(setConfig).toHaveBeenCalledTimes(1));
    // Exactly one key: a patch that carried the rest of the record is how two
    // screens overwrite each other's unsaved work.
    expect(setConfig.mock.calls[0]![0]).toEqual({ dedup_window_secs: 60 });
  });

  it("sends a boolean the same way", async () => {
    const { user } = withUser(<ServiceTab />);
    await user.click(await screen.findByRole("switch", { name: "Notify on capture" }));
    await waitFor(() =>
      expect(setConfig.mock.calls[0]![0]).toEqual({ notify_on_copy: true }),
    );
  });

  it("sends the configured storage quota as its own patch", async () => {
    const { user } = withUser(<ServiceTab />);
    const quota = await screen.findByRole("combobox", { name: "Storage quota" });
    await user.selectOptions(quota, String(5 * 1_073_741_824));

    await waitFor(() =>
      expect(setConfig).toHaveBeenCalledWith({ storage_quota_bytes: 5 * 1_073_741_824 }),
    );
  });

  it.each([
    ["Ignore text larger than", String(16 * 1_048_576), { max_text_size_bytes: 16 * 1_048_576 }],
    ["Ignore images larger than", String(128 * 1_048_576), { max_image_size_bytes: 128 * 1_048_576 }],
    ["Ignore files larger than", String(50 * 1_048_576), { max_file_size_bytes: 50 * 1_048_576 }],
    ["Decoded image memory limit", "100", { max_decoded_image_mb: 100 }],
  ])("patches only %s", async (name, value, patch) => {
    const { user } = withUser(<ServiceTab />);
    const control = await screen.findByRole("combobox", { name });
    await user.selectOptions(control, value);

    await waitFor(() => expect(setConfig).toHaveBeenCalledTimes(1));
    expect(setConfig.mock.calls[0]![0]).toEqual(patch);
  });

  it("persists private mode through the service", async () => {
    const { user } = withUser(<ServiceTab />);
    await user.click(await screen.findByRole("switch", { name: "Private mode" }));
    await waitFor(() => expect(setPrivateMode).toHaveBeenCalledWith(true));
    expect(setConfig).not.toHaveBeenCalled();
  });

  it("updates the accessible private-mode switch before the service settles", async () => {
    let settle: ((value: { private_mode: boolean }) => void) | undefined;
    setPrivateMode.mockImplementation(
      () =>
        new Promise((resolve) => {
          settle = resolve;
        }),
    );
    const { user } = withUser(<ServiceTab />);
    const control = await screen.findByRole("switch", { name: "Private mode" });
    getPrivateMode.mockResolvedValue({ private_mode: true });

    await user.click(control);

    await waitFor(() => expect(control.getAttribute("aria-checked")).toBe("true"));
    expect(control.hasAttribute("disabled")).toBe(true);
    await act(async () => settle?.({ private_mode: true }));
    await waitFor(() => expect(control.hasAttribute("disabled")).toBe(false));
    expect(control.getAttribute("aria-checked")).toBe("true");
  });

  it("offers the persisted private-mode control on Android", async () => {
    getStatus.mockResolvedValue(status({ clipboard_backend: "android-inprocess" }));
    withUser(<ServiceTab />);
    expect(await screen.findByRole("switch", { name: "Private mode" })).toBeTruthy();
  });

  it("settles to the backend echo instead of assuming the requested value", async () => {
    setPrivateMode.mockResolvedValue({ private_mode: false });
    const { user } = withUser(<ServiceTab />);
    const control = await screen.findByRole("switch", { name: "Private mode" });
    await user.click(control);
    await waitFor(() => expect(setPrivateMode).toHaveBeenCalledWith(true));
    await waitFor(() => expect(control.getAttribute("aria-checked")).toBe("false"));
  });
});

describe("payload-limit validation", () => {
  it("offers the exact 64 KiB, 1 MiB, 100 MiB, and 5000 ms boundaries", async () => {
    withUser(<ServiceTab />);
    const values = async (name: string) =>
      [...((await screen.findByRole("combobox", { name })) as HTMLSelectElement).options].map(
        (option) => option.value,
      );

    expect(await values("Check the clipboard every")).toContain("5000");
    expect(await values("Ignore text larger than")).toContain(String(64 * 1_024));
    expect(await values("Ignore images larger than")).toContain(String(1_048_576));
    expect(await values("Ignore files larger than")).toContain(String(100 * 1_048_576));
    expect(await values("Decoded image memory limit")).toContain("1");
  });

  it("announces an out-of-range service value accessibly", async () => {
    getConfig.mockResolvedValue(applied({ max_file_size_bytes: 101 * 1_048_576 }));
    withUser(<ServiceTab />);
    const file = await screen.findByRole("combobox", {
      name: "Ignore files larger than",
    });
    expect(file.getAttribute("aria-invalid")).toBe("true");
    const error = await screen.findByRole("alert");
    expect(error.textContent).toContain("1 MB through 100 MB");
    expect(file.getAttribute("aria-errormessage")).toBe(error.id);
  });
});

describe("liveness", () => {
  /**
   * `lan_visibility` was marked `NeedsRestart`, and the row carried a badge
   * saying so that named the field in the component. Discovery became
   * startable behind `&self` in 60a2da93 and the daemon has applied the change
   * on the spot ever since; the badge went on claiming otherwise, because
   * nothing tied it to the answer the service gives.
   */
  it("claims no restart for a field the service applies live", async () => {
    withUser(<ServiceTab />);
    const lan = await screen.findByRole("switch", { name: "Visible on this network" });
    expect(lan.closest("div.flex-wrap")?.textContent).not.toContain("restart");
  });

  it("offers the restart once the service says the change is waiting on one", async () => {
    // No field is marked `NeedsRestart` today, so which field this names is
    // arbitrary. What is asserted is that the offer follows the service's list
    // rather than a field the screen names for itself.
    setConfig.mockResolvedValue(applied({ sound_on_copy: true }, ["sound_on_copy"]));
    const { user } = withUser(<ServiceTab />);
    await user.click(await screen.findByRole("switch", { name: "Sound on capture" }));
    expect(
      await screen.findByRole("button", { name: "Restart the service" }),
    ).toBeTruthy();
  });

  it("offers no restart for a change that took effect immediately", async () => {
    const { user } = withUser(<ServiceTab />);
    await user.click(await screen.findByRole("switch", { name: "Sound on capture" }));
    await waitFor(() => expect(setConfig).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: "Restart the service" })).toBeNull();
  });
});

describe("the sensitive-content sweep", () => {
  it("has a control, and it is off out of the box", async () => {
    withUser(<ServiceTab />);
    const ttl = await screen.findByRole("combobox", {
      name: "Delete detected secrets after",
    });
    expect((ttl as HTMLSelectElement).value).toBe("0");
    expect(screen.getByText(/kept until you delete them/)).toBeTruthy();
  });

  /** Turning it on is turning on an unrecoverable delete. The control has to
   *  say that where the user is looking (AGENTS.md rule 4). */
  it("states what turning it on costs, at the control", async () => {
    getConfig.mockResolvedValue(applied({ sensitive_ttl_secs: 30 }));
    withUser(<ServiceTab />);
    expect(
      await screen.findByText(/deleted without asking and cannot be recovered/i),
    ).toBeTruthy();
  });

  /** The other half, now that `EventData::swept` exists: an unrecoverable
   *  delete has to say where it will be reported, and the note must not go on
   *  claiming it is not. */
  it("says where an automatic deletion is reported", async () => {
    getConfig.mockResolvedValue(applied({ sensitive_ttl_secs: 30 }));
    withUser(<ServiceTab />);
    expect(
      await screen.findByText(/announced and counted in Diagnostics/i),
    ).toBeTruthy();
  });

  /** Manifest 01 §4 and manifest 07 §6.2 both give 30 as the value; the control
   *  has to be able to reach it, or the recorded route back to that default is
   *  closed. */
  it("offers the 30 seconds the manifests name", async () => {
    withUser(<ServiceTab />);
    const ttl = await screen.findByRole("combobox", {
      name: "Delete detected secrets after",
    });
    expect(
      [...(ttl as HTMLSelectElement).options].map((option) => option.value),
    ).toContain("30");
  });
});

it("names no path anywhere (INV-12)", async () => {
  const { container } = withUser(<ServiceTab />);
  await screen.findByRole("combobox", { name: "Check the clipboard every" });
  expect(container.innerHTML).not.toMatch(/\/Users\/|\/home\/|~\/|\.sock/);
});
