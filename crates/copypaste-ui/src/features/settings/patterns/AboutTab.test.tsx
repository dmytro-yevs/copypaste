import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { t } from "@/i18n";
import { SETTINGS_SEARCH_ITEMS } from "@/features/settings/model/settingsSearchIndex";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import { captureSnapshot } from "@/test/harness";
import { useUi } from "@/store/ui";
import { AboutTab } from "./AboutTab";

const mocks = vi.hoisted(() => ({
  status: undefined as unknown,
  statusError: false,
  capture: undefined as unknown,
  captureError: false,
}));

vi.mock("@/features/settings/components/useUpdateSetting", () => ({
  useUpdateSetting: () => ({
    field: { kind: "readonly", definition: settingDefinition("about", "settings.about.updates.title"), value: "Up to date" },
    dialog: null,
  }),
}));

vi.mock("@/hooks/useStatus", () => ({
  statusService: {},
  useStatus: () => ({ data: mocks.status, error: mocks.statusError ? new Error("offline") : null, isError: mocks.statusError }),
}));

vi.mock("@/hooks/useCapture", () => ({
  useCaptureState: () => ({ data: mocks.capture, isError: mocks.captureError }),
}));

vi.mock("@/lib/appVersion", () => ({
  appVersion: () => Promise.resolve("2.0.0-test"),
}));

beforeEach(() => {
  vi.stubGlobal("__COPYPASTE_APP_VERSION__", "2.0.0-test");
  mocks.status = undefined;
  mocks.statusError = false;
  mocks.capture = undefined;
  mocks.captureError = false;
});

afterEach(() => {
  vi.unstubAllGlobals();
  useUi.setState({ onboardingOpen: false });
});

describe("About settings", () => {
  it("names capture as checking while the snapshot is loading", () => {
    render(<AboutTab />);
    expect(screen.getByRole("status", { name: "Checking…" })).toBeTruthy();
  });

  it("renders a destination for every About search entry", () => {
    const { container } = render(<AboutTab />);
    const targets = new Set(
      [...container.querySelectorAll<HTMLElement>("[data-settings-search-target]")]
        .map((element) => element.dataset.settingsSearchTarget),
    );

    for (const item of SETTINGS_SEARCH_ITEMS.filter(({ tab }) => tab === "about")) {
      const title = String(t(item.title as never));
      const section = item.section === undefined
        ? undefined
        : String(t(item.section as never));
      const candidates = [
        `row:${title}`,
        `section:${title}`,
        section === undefined ? undefined : `section:${section}`,
      ];
      expect(candidates.some((candidate) => candidate && targets.has(candidate))).toBe(true);
    }
  });

  it("opens the welcome flow without changing clipboard history", () => {
    useUi.setState({ onboardingOpen: false });
    render(<AboutTab />);

    const welcome = screen.getByRole("button", { name: "Open welcome" });
    const reset = screen.getByRole("button", { name: "Reset preferences" });

    expect(welcome.closest("section")?.className).toBe(
      reset.closest("section")?.className,
    );
    expect(welcome.className).not.toBe(reset.className);
    fireEvent.click(welcome);

    expect(useUi.getState().onboardingOpen).toBe(true);
  });

  it.each([
    [true, "Running"],
    [false, "Paused"],
  ])("shows desktop capture from daemon running=%s", (running, label) => {
    mocks.status = { version: "2.0", capture_running: running, clipboard_backend: "system", protocol_version: 1, item_count: 0 };
    mocks.capture = captureSnapshot({ rung: "desktop", headline: "Capturing everything you copy." });
    render(<AboutTab />);

    expect(screen.getByText(label)).toBeTruthy();
    expect(screen.queryByText(/Android Share/)).toBeNull();
  });

  it("shows Android background status and remaining capture modes", () => {
    mocks.status = { version: "2.0", capture_running: false, clipboard_backend: "system", protocol_version: 1, item_count: 0 };
    mocks.capture = captureSnapshot({
      rung: "in_app",
      health: { state: "disabled" },
      headline: "Background capture is off.",
      detail: "CopyPaste still saves in-app and shared copies.",
    });
    render(<AboutTab />);

    expect(screen.getByText(/Background capture is off.*Android Share.*Quick Settings tile/)).toBeTruthy();
    expect(screen.queryByText("Paused")).toBeNull();
  });

  it("shows Android background capture working from its snapshot", () => {
    mocks.status = { version: "2.0", capture_running: false, clipboard_backend: "system", protocol_version: 1, item_count: 0 };
    mocks.capture = captureSnapshot({ headline: "Capturing from every app." });
    render(<AboutTab />);

    expect(screen.getByText("Capturing from every app.")).toBeTruthy();
    expect(screen.queryByText("Paused")).toBeNull();
    expect(screen.queryByText(/Android Share/)).toBeNull();
  });

  it("shows the Android setup state without claiming background capture works", () => {
    mocks.capture = captureSnapshot({
      rung: "in_app",
      health: { state: "not_granted", reason: "not_installed" },
      headline: "Background capture isn't set up.",
    });
    render(<AboutTab />);

    expect(screen.getByText(/Background capture isn't set up.*Android Share/)).toBeTruthy();
  });

  it("shows a refused Android background read as unavailable for that mode", () => {
    mocks.capture = captureSnapshot({
      rung: "in_app",
      health: { state: "granted_not_working", reason: "read_refused" },
      headline: "Background capture isn't working.",
    });
    render(<AboutTab />);

    expect(screen.getByText(/Background capture isn't working.*Android Share/)).toBeTruthy();
  });

  it("shows unavailable when desktop status or capture cannot be read", () => {
    mocks.capture = captureSnapshot({ rung: "desktop" });
    mocks.statusError = true;
    const { rerender } = render(<AboutTab />);
    expect(screen.getByText("Unavailable")).toBeTruthy();

    mocks.statusError = false;
    mocks.captureError = true;
    rerender(<AboutTab />);
    expect(screen.getByText("Unavailable")).toBeTruthy();
  });
});
