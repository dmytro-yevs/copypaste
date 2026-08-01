/**
 * The service's settings, which had no interface at all until this tab.
 *
 * Three of these are the properties the daemon's own contract is built on, seen
 * from the screen that uses it: a value that round-trips, a write that carries
 * only the field it names, and the one field whose change does not take effect
 * until the service restarts. They were previously exercised only by
 * `e2e/tests/daemon-config.e2e.test.ts` driving the CLI, because no Tauri
 * command routed `GetConfig` or `SetConfig`.
 *
 * The fourth is `sensitive_ttl_secs`. It ships off, and the reason recorded on
 * the field is that v2 had nowhere to say the sweep had happened — so the
 * assertions here are about what the control says, not about what it stores.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { ServiceTab } from "@/components/settings/ServiceTab";
import type { ConfigApplied, ConfigData, ConfigPatch } from "@/lib/ipc";
import { status, withUser } from "@/test/harness";

const getConfig = vi.fn();
const setConfig = vi.fn();
const getStatus = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    getConfig: () => getConfig(),
    setConfig: (patch: ConfigPatch) => setConfig(patch),
    getStatus: () => getStatus(),
  };
});

function config(over: Partial<ConfigData> = {}): ConfigData {
  return {
    private_mode: false,
    poll_interval_ms: 500,
    history_limit: 10_000,
    retention_days: 0,
    dedup_window_secs: 30,
    max_item_bytes: 4 * 1_048_576,
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
  getStatus.mockReset().mockResolvedValue(status());
});

afterEach(() => vi.restoreAllMocks());

describe("reading the service's settings", () => {
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

  it("says so plainly on a build that has no service to configure", async () => {
    getConfig.mockRejectedValue("not available in this build");
    withUser(<ServiceTab />);
    expect(await screen.findByText(/can't change the background service/)).toBeTruthy();
  });
});

describe("writing one", () => {
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

  it("persists private mode through the service", async () => {
    const { user } = withUser(<ServiceTab />);
    await user.click(await screen.findByRole("switch", { name: "Private mode" }));
    await waitFor(() =>
      expect(setConfig.mock.calls[0]![0]).toEqual({ private_mode: true }),
    );
  });

  it("does not offer an unpersisted private-mode control on Android", async () => {
    getStatus.mockResolvedValue(status({ clipboard_backend: "android-inprocess" }));
    withUser(<ServiceTab />);
    expect(await screen.findByRole("combobox", { name: "Check the clipboard every" })).toBeTruthy();
    expect(screen.queryByRole("switch", { name: "Private mode" })).toBeNull();
  });
});

describe("liveness (the one field marked NeedsRestart)", () => {
  it("marks the field itself, before anyone touches it", async () => {
    withUser(<ServiceTab />);
    const lan = await screen.findByRole("switch", { name: "Visible on this network" });
    // The badge has to be beside this control, not in a footnote: a rule read
    // after the switch appeared to do nothing is a rule read too late.
    const row = lan.closest("div.flex-wrap");
    expect(row?.textContent).toContain("Needs a restart");
  });

  it("offers the restart once the service says the change is waiting on one", async () => {
    setConfig.mockResolvedValue(applied({ lan_visibility: false }, ["lan_visibility"]));
    const { user } = withUser(<ServiceTab />);
    await user.click(
      await screen.findByRole("switch", { name: "Visible on this network" }),
    );
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
   *  say that where the user is looking (CLAUDE.md rule 4). */
  it("states what turning it on costs, at the control", async () => {
    getConfig.mockResolvedValue(applied({ sensitive_ttl_secs: 30 }));
    withUser(<ServiceTab />);
    expect(
      await screen.findByText(
        /deleted without asking.*Nothing that is deleted can be brought back/s,
      ),
    ).toBeTruthy();
  });

  /** The other half, now that `EventData::swept` exists: an unrecoverable
   *  delete has to say where it will be reported, and the note must not go on
   *  claiming it is not. */
  it("says where an automatic deletion is reported", async () => {
    getConfig.mockResolvedValue(applied({ sensitive_ttl_secs: 30 }));
    withUser(<ServiceTab />);
    expect(
      await screen.findByText(/announced as it happens.*Diagnostics tab/s),
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
