/**
 * The states a user can get stuck in.
 *
 * The one this file exists for is the reboot: Shizuku is stopped by every
 * restart, so "start it again" happens for as long as the user keeps rung 2. It
 * has to read as a routine prompt with one tap behind it — not as a failure,
 * and not as something to be interrupted about.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { CaptureSetup } from "@/components/capture/CaptureSetup";
import type { CaptureSnapshot } from "@/lib/ipc";
import * as platform from "@/lib/platform";
import { captureSnapshot, probe, withUser } from "@/test/harness";

const captureState = vi.fn();
const captureArm = vi.fn();
const captureRefresh = vi.fn();
const captureSetEnabled = vi.fn();
const getConfig = vi.fn();
const listInstalledSourceApps = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    captureState: () => captureState(),
    captureArm: () => captureArm(),
    captureRefresh: () => captureRefresh(),
    captureSetEnabled: (enabled: boolean) => captureSetEnabled(enabled),
    getConfig: () => getConfig(),
    listInstalledSourceApps: () => listInstalledSourceApps(),
    listItems: () => Promise.resolve({
      items: [],
      total: 0,
      skipped_undecryptable: 0,
      next_cursor: null,
    }),
  };
});

/** The exact sentences `capture::messages` produces for the two states a
 *  restart leaves behind. Quoted rather than paraphrased: if Rust's wording
 *  changes, these fixtures are where that shows up. */
const SHIZUKU_STOPPED: CaptureSnapshot = captureSnapshot({
  rung: "in_app",
  shizuku: probe({ running: false, permission: false }),
  health: { state: "not_granted", reason: "not_running" },
  nextStep: "start_shizuku",
  headline: "Background capture isn't set up.",
  detail:
    "Shizuku isn't running. Android stops it on every restart, so this is expected after a reboot — start it again to resume capturing from other apps.",
  lastReadOkAt: null,
});

const LISTENER_LOST: CaptureSnapshot = captureSnapshot({
  rung: "in_app",
  health: { state: "granted_not_working", reason: "not_armed" },
  nextStep: "arm",
  headline: "Background capture stopped.",
  detail: "CopyPaste is only saving what you copy inside the app. Tap to restart.",
  lastReadOkAt: null,
});

beforeEach(() => {
  captureState.mockReset().mockResolvedValue(captureSnapshot());
  captureArm.mockReset().mockResolvedValue(captureSnapshot());
  captureRefresh.mockReset().mockResolvedValue(captureSnapshot());
  captureSetEnabled.mockReset().mockResolvedValue(captureSnapshot());
  getConfig.mockReset().mockResolvedValue({
    config: { excluded_app_bundle_ids: [] },
    restart_required: [],
  });
  listInstalledSourceApps.mockReset().mockResolvedValue([
    { package_id: "com.example.notes", label: "Notes" },
  ]);
});

afterEach(() => vi.restoreAllMocks());

async function show(snapshot: CaptureSnapshot) {
  captureState.mockResolvedValue(snapshot);
  const rendered = withUser(<CaptureSetup />);
  await waitFor(() => expect(screen.getByText(snapshot.headline)).toBeTruthy());
  return rendered;
}

describe("a restart is not a failure", () => {
  it.each([
    ["shizuku stopped by the reboot", SHIZUKU_STOPPED],
    ["the listener gone with the binder", LISTENER_LOST],
  ])("states %s without interrupting and without the fault tone", async (_name, snapshot) => {
    const { container } = await show(snapshot);

    // A11Y-5: informational, so a screen reader is not interrupted every time
    // the phone reboots.
    expect(screen.queryByRole("alert")).toBeNull();
    const card = container.querySelector("[data-tone]");
    expect(card?.getAttribute("data-tone")).toBe("attention");
    expect(card?.getAttribute("role")).toBe("status");
  });

  it.each([
    ["shizuku stopped by the reboot", SHIZUKU_STOPPED, /check again/i],
    ["the listener gone with the binder", LISTENER_LOST, /turn on background capture/i],
  ])("leaves one tap in front of the user (%s)", async (_name, snapshot, label) => {
    await show(snapshot);
    expect(screen.getByRole("button", { name: label })).toBeTruthy();
  });

  /** The state machine's own sentence, verbatim. The view does not get to
   *  soften it or replace it (ADR-0005). */
  it("shows the snapshot's explanation rather than one of its own", async () => {
    await show(SHIZUKU_STOPPED);
    expect(screen.getByText(SHIZUKU_STOPPED.detail!)).toBeTruthy();
  });

  it("re-reads the platform rather than pretending to start Shizuku", async () => {
    const { user } = await show(SHIZUKU_STOPPED);
    await user.click(screen.getByRole("button", { name: /check again/i }));
    expect(captureRefresh).toHaveBeenCalledTimes(1);
    expect(captureArm).not.toHaveBeenCalled();
  });

  /** One command covers both asking for the permission and registering the
   *  listener, so the button is the same one either way. */
  it("arms with the single command when the listener is what is missing", async () => {
    const { user } = await show(LISTENER_LOST);
    await user.click(screen.getByRole("button", { name: /turn on background capture/i }));
    expect(captureArm).toHaveBeenCalledTimes(1);
  });

  it("shows which step the device is standing on", async () => {
    const { container } = await show(SHIZUKU_STOPPED);
    const step = container.querySelector('[data-step="start"]');
    // Not by colour alone: the step says where it stands in words (A11Y-10).
    expect(step?.textContent).toContain("Next");
    expect(container.querySelector('[data-step="install"]')?.textContent).toContain("Done");
  });
});

describe("a refused read", () => {
  const REFUSED: CaptureSnapshot = captureSnapshot({
    rung: "in_app",
    health: { state: "granted_not_working", reason: "read_refused" },
    nextStep: "none",
    headline: "Background capture isn't working.",
    detail:
      "Shizuku is running, but reading the clipboard was refused. Only what you copy inside the app is being saved.",
    lastReadOkAt: null,
  });

  /** `next_step` refuses to name an action here because none of them would
   *  help. The screen must not invent one. */
  it("offers no button that would do nothing", async () => {
    await show(REFUSED);
    for (const label of [/turn on background capture/i, /ask shizuku/i, /check again/i]) {
      expect(screen.queryByRole("button", { name: label })).toBeNull();
    }
  });

  /** The one state nothing can fix is also the one worth interrupting for. */
  it("is the state that does interrupt", async () => {
    const { container } = await show(REFUSED);
    expect(container.querySelector("[data-tone]")?.getAttribute("role")).toBe("alert");
  });
});

describe("a platform that cannot reach rung 2", () => {
  it("shows no ladder and asks for nothing", async () => {
    await show(
      captureSnapshot({
        rung: "in_app",
        shizuku: probe({ supported: false, installed: false, running: false, permission: false }),
        health: { state: "not_granted", reason: "unsupported" },
        nextStep: "none",
        headline: "Background capture isn't set up.",
        detail:
          "Capturing from other apps needs Android 11 or later. Everything else works here.",
        lastReadOkAt: null,
      }),
    );
    expect(screen.queryByRole("list", { name: /setup steps/i })).toBeNull();
    expect(screen.queryByRole("switch")).toBeNull();
  });
});

describe("macOS", () => {
  /** Every mutating capture command refuses there, so a control would be a
   *  control that cannot work. */
  it("states the one thing it has to state and offers no controls", async () => {
    await show(
      captureSnapshot({
        rung: "desktop",
        health: { state: "working" },
        nextStep: "none",
        headline: "Capturing everything you copy.",
        detail: null,
      }),
    );
    expect(screen.queryByRole("switch")).toBeNull();
    expect(screen.queryByRole("button", { name: /save the clipboard now/i })).toBeNull();
  });
});

describe("copies that were taken and not saved", () => {
  it("says so rather than leaving the user to find the hole", async () => {
    await show(captureSnapshot({ rung: "in_app", droppedClips: 3 }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("3 copies");
  });
});

describe("in every state", () => {
  it.each([
    ["stopped", SHIZUKU_STOPPED],
    ["lost", LISTENER_LOST],
    ["working", captureSnapshot({ rung: "in_app" })],
  ])("never renders a filesystem path (%s)", async (_name, snapshot) => {
    const { container } = await show(snapshot);
    expect(container.innerHTML).not.toMatch(/\/Users\/|\/home\/|\.sock/);
  });

  it("keeps the offscreen Android exclusion input out of the accessibility tree", async () => {
    vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(true);
    const { user } = await show(captureSnapshot({ rung: "in_app" }));
    const disclosure = await screen.findByRole("button", {
      name: "Exclude apps from capture",
    });
    const controlsId = disclosure.getAttribute("aria-controls");

    expect(disclosure.getAttribute("aria-expanded")).toBe("false");
    expect(controlsId).toBeTruthy();
    expect(document.getElementById(controlsId!)).toBeNull();
    expect(document.getElementById("android-exclusion-search")).toBeNull();

    await user.click(disclosure);

    const search = await screen.findByRole("textbox", { name: "Search installed apps" });
    expect(disclosure.getAttribute("aria-expanded")).toBe("true");
    expect(document.getElementById(controlsId!)).not.toBeNull();
    expect(search.id).toBe("android-exclusion-search");
    expect(search.getAttribute("placeholder")).toBe("Search installed apps");
    expect(search.className).toContain("min-h-[var(--tap-min)]");
    expect(document.getElementById(controlsId!)?.contains(search)).toBe(true);
  });
});
