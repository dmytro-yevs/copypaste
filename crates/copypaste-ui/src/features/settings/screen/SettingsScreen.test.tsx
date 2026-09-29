import { afterEach, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";

import { TooltipProvider } from "@/components/ui";
import {
  disclosureRevealKey,
  type SettingsDisclosureReveal,
} from "@/features/settings/model/settingsSearchIndex";
import { CloudSyncSettings } from "@/features/settings/patterns/CloudSyncSettings";
import type { CloudStatusData } from "@/lib/ipc";
import { useUi } from "@/store/ui";
import { withClient } from "@/test/harness";
import { SettingsScreen } from "./SettingsScreen";

const viewport = vi.hoisted(() => ({
  width: 390,
  height: 800,
  pointer: "coarse" as const,
  sizeClass: "compact" as const,
}));
const mockContent = vi.hoisted(() => ({ showAdvancedField: true }));
const cloudIpc = vi.hoisted(() => ({ getCloudStatus: vi.fn() }));

vi.mock("@/lib/ipc", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/ipc")>()),
  getCloudStatus: () => cloudIpc.getCloudStatus(),
}));

vi.mock("@/hooks/useViewportMetrics", () => ({
  useViewportMetrics: () => viewport,
  useObservedElementSize: () => ({
    ref: () => {},
    width: viewport.width,
    height: viewport.height,
  }),
}));

vi.mock("@/hooks/useSizeClass", () => ({
  useSizeClass: () => viewport.sizeClass,
}));

vi.mock("@/features/settings/patterns/settingsTabs", () => ({
  renderPreferenceSection: (section: string, controller: {
    diagnosticsView?: string;
    disclosureReveal?: SettingsDisclosureReveal;
    onOpenEvents?: () => void;
    onBackFromEvents?: () => void;
  }) => (
    <div
      data-testid={`settings-section-${section}`}
      data-settings-search-target={
        section === "clipboard" ? "row:Group by device" : undefined
      }
    >
      {section === "clipboard" ? (
        <>
          <button title="Help with Group by device">?</button>
          <div data-settings-control>
            <button aria-label="Group by device">Toggle group by device</button>
          </div>
        </>
      ) : null}
      {section === "diagnostics" ? controller.diagnosticsView === "runtime-events" ? (
        <>
          {controller.onBackFromEvents ? <button onClick={controller.onBackFromEvents}>Back to Diagnostics</button> : null}
          <div data-settings-search-target="row:Runtime events">
            <input type="search" aria-label="Search runtime events" />
          </div>
        </>
      ) : <button onClick={controller.onOpenEvents}>Open runtime events</button> : null}
      {section === "clipboard" ? (
        <details open={Boolean(disclosureRevealKey(controller.disclosureReveal, "clipboard-advanced"))}>
          <summary>Advanced capture settings</summary>
          {mockContent.showAdvancedField ? (
            <div data-settings-search-target="row:Check the clipboard every">
              <select aria-label="Check the clipboard every"><option>500 ms</option></select>
            </div>
          ) : null}
        </details>
      ) : null}
      {section === "cloud-sync" ? (
        <CloudSyncSettings revealAdvancedKey={disclosureRevealKey(controller.disclosureReveal, "cloud-server")} />
      ) : null}
    </div>
  ),
}));

afterEach(() => {
  vi.restoreAllMocks();
  mockContent.showAdvancedField = true;
  cloudIpc.getCloudStatus.mockReset();
  useUi.setState({ settingsTab: null });
  Object.assign(viewport, {
    width: 390,
    height: 800,
    pointer: "coarse",
    sizeClass: "compact",
  });
});

function cloudStatus(configured: boolean): CloudStatusData {
  return {
    configured,
    signed_in: false,
    key_ready: false,
    email: null,
    last_sync_ms: null,
    last_error: null,
    poll_interval_secs: 60,
    unreadable_uploads: 0,
  };
}

it("resets compact settings scroll when opening and leaving a detail", async () => {
  render(
    <TooltipProvider>
      <SettingsScreen />
    </TooltipProvider>,
  );

  expect(screen.getByRole("heading", { name: "Settings" })).toBeTruthy();
  const menu = screen.getByRole("navigation", {
    name: "Settings sections",
  });
  const viewport = menu.parentElement?.parentElement;
  expect(viewport).toBeInstanceOf(HTMLDivElement);
  if (!(viewport instanceof HTMLDivElement)) return;

  viewport.scrollTop = 240;
  fireEvent.click(screen.getByRole("button", { name: /^Appearance/ }));

  expect(await screen.findByTestId("settings-section-appearance")).toBeTruthy();
  expect(viewport.scrollTop).toBe(0);

  viewport.scrollTop = 240;
  fireEvent.click(screen.getByRole("button", { name: "Back to Settings" }));

  expect(await screen.findByRole("navigation", {
    name: "Settings sections",
  })).toBeTruthy();
  expect(viewport.scrollTop).toBe(0);
});

it("does not expose Data transfer as a preference destination", () => {
  render(
    <TooltipProvider>
      <SettingsScreen />
    </TooltipProvider>,
  );

  expect(screen.queryByRole("button", { name: /Data transfer/i })).toBeNull();
  expect(screen.getByRole("button", { name: /^Storage & history/ })).toBeTruthy();
});

it("opens Storage & history for an old data-transfer selection", async () => {
  useUi.setState({ settingsTab: "data-transfer" });
  render(
    <TooltipProvider>
      <SettingsScreen />
    </TooltipProvider>,
  );

  expect(await screen.findByTestId("settings-section-storage")).toBeTruthy();
  expect(useUi.getState().settingsTab).toBeNull();
});

it("opens old Runtime events destinations inside Diagnostics and steps back through both levels", async () => {
  useUi.setState({ settingsTab: "runtime-events" });
  render(<TooltipProvider><SettingsScreen /></TooltipProvider>);

  expect(await screen.findByRole("searchbox", { name: "Search runtime events" })).toBeTruthy();
  await waitFor(() => expect(window.history.state?.copypasteSettingsLevel?.path)
    .toEqual(["diagnostics", "runtime-events"]));
  expect(screen.queryByRole("button", { name: /^Runtime events/ })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Back to Diagnostics" }));
  expect(await screen.findByRole("button", { name: "Open runtime events" })).toBeTruthy();
  await waitFor(() => expect(window.history.state?.copypasteSettingsLevel?.path)
    .toEqual(["diagnostics"]));
  fireEvent.click(screen.getByRole("button", { name: "Back to Settings" }));
  expect(await screen.findByRole("navigation", { name: "Settings sections" })).toBeTruthy();
  await waitFor(() => expect(window.history.state?.copypasteSettingsLevel?.path).toEqual([]));
  act(() => window.history.forward());
  expect(await screen.findByRole("button", { name: "Open runtime events" })).toBeTruthy();
  await waitFor(() => expect(window.history.state?.copypasteSettingsLevel?.path)
    .toEqual(["diagnostics"]));
  act(() => window.history.forward());
  expect(await screen.findByRole("searchbox", { name: "Search runtime events" })).toBeTruthy();
});

it("search opens a collapsed advanced group and focuses its field", async () => {
  render(<TooltipProvider><SettingsScreen /></TooltipProvider>);
  const searchbox = screen.getByRole("searchbox", { name: "Search settings" });
  fireEvent.change(searchbox, { target: { value: "Check the clipboard every" } });
  fireEvent.click(await screen.findByRole("option", { name: /Check the clipboard every/ }));

  const field = await screen.findByRole("combobox", { name: "Check the clipboard every" });
  await waitFor(() => {
    expect(field.closest("details")?.open).toBe(true);
    expect(document.activeElement).toBe(field);
  });
});

it("focuses the setting control before adjacent helper buttons", async () => {
  render(<TooltipProvider><SettingsScreen /></TooltipProvider>);
  const searchbox = screen.getByRole("searchbox", { name: "Search settings" });
  fireEvent.change(searchbox, { target: { value: "Group by device" } });
  fireEvent.click(await screen.findByRole("option", { name: /Group by device/ }));

  const control = await screen.findByRole("button", { name: "Group by device" });
  await waitFor(() => expect(document.activeElement).toBe(control));
});

it("uses the terse screen header on settings and its compact subpages", async () => {
  render(<TooltipProvider><SettingsScreen /></TooltipProvider>);
  const homeHeader = screen.getByRole("heading", { name: "Settings" }).closest("header");

  expect(homeHeader?.querySelector("p")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: /^Appearance/ }));

  const detailHeader = (await screen.findByRole("heading", { name: "Appearance" })).closest("header");
  expect(detailHeader?.querySelector("p")).toBeNull();
});

it.each([
  ["Server URL", true],
  ["Publishable key", false],
] as const)("search opens the cloud server and focuses %s after status loads", async (fieldName, configured) => {
  let resolveStatus!: (status: CloudStatusData) => void;
  cloudIpc.getCloudStatus.mockImplementation(() => new Promise<CloudStatusData>((resolve) => {
    resolveStatus = resolve;
  }));
  withClient(<TooltipProvider><SettingsScreen /></TooltipProvider>);
  const searchbox = screen.getByRole("searchbox", { name: "Search settings" });
  fireEvent.change(searchbox, { target: { value: fieldName } });
  fireEvent.click(await screen.findByRole("option", { name: new RegExp(fieldName) }));

  await waitFor(() => expect(cloudIpc.getCloudStatus).toHaveBeenCalledTimes(1));
  expect(screen.queryByRole("textbox", { name: fieldName })?.closest("details")?.open).not.toBe(true);
  await act(async () => resolveStatus(cloudStatus(configured)));
  const field = await screen.findByRole("textbox", { name: fieldName });
  await waitFor(() => {
    expect(field.closest("details")?.open).toBe(true);
    expect(document.activeElement).toBe(field);
  });
});

it("does not carry a clipboard search reveal into the cloud server", async () => {
  cloudIpc.getCloudStatus.mockResolvedValue(cloudStatus(true));
  withClient(<TooltipProvider><SettingsScreen /></TooltipProvider>);
  const searchbox = screen.getByRole("searchbox", { name: "Search settings" });
  fireEvent.change(searchbox, { target: { value: "polling" } });
  fireEvent.click(await screen.findByRole("option", { name: /Check the clipboard every/ }));
  const polling = await screen.findByRole("combobox", { name: "Check the clipboard every" });
  await waitFor(() => {
    expect(polling.closest("details")?.open).toBe(true);
    expect(document.activeElement).toBe(polling);
  });

  act(() => useUi.setState({ settingsTab: "cloud-sync" }));
  const cloud = await screen.findByText("Advanced · Self-hosted cloud server");
  expect(cloud.closest("details")?.open).toBe(false);
});

it("does not carry a cloud search reveal into advanced clipboard settings", async () => {
  cloudIpc.getCloudStatus.mockResolvedValue(cloudStatus(true));
  withClient(<TooltipProvider><SettingsScreen /></TooltipProvider>);
  const searchbox = screen.getByRole("searchbox", { name: "Search settings" });
  fireEvent.change(searchbox, { target: { value: "Server URL" } });
  fireEvent.click(await screen.findByRole("option", { name: /Server URL/ }));
  const url = await screen.findByRole("textbox", { name: "Server URL" });
  await waitFor(() => expect(url.closest("details")?.open).toBe(true));

  act(() => useUi.setState({ settingsTab: "clipboard" }));
  const clipboard = await screen.findByText("Advanced capture settings");
  expect(clipboard.closest("details")?.open).toBe(false);
});

it("waits for a service field to load before focusing a search destination", async () => {
  mockContent.showAdvancedField = false;
  const { rerender } = render(<TooltipProvider><SettingsScreen /></TooltipProvider>);
  const searchbox = screen.getByRole("searchbox", { name: "Search settings" });
  fireEvent.change(searchbox, { target: { value: "Check the clipboard every" } });
  fireEvent.click(await screen.findByRole("option", { name: /Check the clipboard every/ }));
  expect(screen.queryByRole("combobox", { name: "Check the clipboard every" })).toBeNull();

  mockContent.showAdvancedField = true;
  rerender(<TooltipProvider><SettingsScreen /></TooltipProvider>);
  const field = await screen.findByRole("combobox", { name: "Check the clipboard every" });
  await waitFor(() => expect(document.activeElement).toBe(field));
});

it("search opens Runtime events inside Diagnostics and focuses event search", async () => {
  render(<TooltipProvider><SettingsScreen /></TooltipProvider>);
  const searchbox = screen.getByRole("searchbox", { name: "Search settings" });
  fireEvent.change(searchbox, { target: { value: "Runtime events" } });
  fireEvent.click(await screen.findByRole("option", { name: /Runtime events/ }));

  const eventSearch = await screen.findByRole("searchbox", { name: "Search runtime events" });
  await waitFor(() => expect(document.activeElement).toBe(eventSearch));
});

it("keeps a selected search destination highlighted long enough to find", async () => {
  const timeout = vi.spyOn(window, "setTimeout");
  render(
    <TooltipProvider>
      <SettingsScreen />
    </TooltipProvider>,
  );

  const searchbox = screen.getByRole("searchbox", { name: "Search settings" });
  fireEvent.change(searchbox, { target: { value: "Group by device" } });
  fireEvent.keyDown(searchbox, { key: "ArrowDown" });
  await waitFor(() => {
    expect(searchbox.getAttribute("aria-activedescendant")).toBeTruthy();
  });
  fireEvent.keyDown(searchbox, { key: "Enter" });

  const target = await screen.findByTestId("settings-section-clipboard");
  await waitFor(() => {
    expect(target.dataset.settingsSearchHighlight).toBe("true");
  });
  expect(
    target.style.getPropertyValue("--settings-search-highlight-duration"),
  ).toBe("3200ms");
  expect(timeout).toHaveBeenCalledWith(expect.any(Function), 3_200);
});

it("connects settings tabs to their panels and supports roving keyboard navigation", async () => {
  Object.assign(viewport, {
    width: 1_024,
    height: 800,
    pointer: "fine",
    sizeClass: "expanded",
  });
  render(
    <TooltipProvider>
      <SettingsScreen />
    </TooltipProvider>,
  );

  const tablist = screen.getByRole("tablist", { name: "Settings sections" });
  const appearance = screen.getByRole("tab", { name: "Appearance" });
  const panel = screen.getByRole("tabpanel");
  expect(tablist.contains(appearance)).toBe(true);
  expect(tablist.getAttribute("aria-orientation")).toBe("vertical");
  expect(panel.getAttribute("aria-labelledby")).toBe(appearance.id);
  expect(appearance.getAttribute("aria-controls")).toBe(panel.id);

  appearance.focus();
  fireEvent.keyDown(appearance, { key: "ArrowDown" });
  const clipboard = screen.getByRole("tab", { name: "Clipboard behavior" });
  await waitFor(() => {
    expect(document.activeElement).toBe(clipboard);
    expect(clipboard.getAttribute("aria-selected")).toBe("true");
  });

  fireEvent.keyDown(clipboard, { key: "End" });
  const about = screen.getByRole("tab", { name: "About" });
  await waitFor(() => {
    expect(document.activeElement).toBe(about);
    expect(about.getAttribute("aria-selected")).toBe("true");
  });

  fireEvent.keyDown(about, { key: "Home" });
  await waitFor(() => {
    expect(document.activeElement).toBe(appearance);
    expect(appearance.getAttribute("aria-selected")).toBe("true");
  });

  fireEvent.keyDown(appearance, { key: "ArrowUp" });
  await waitFor(() => {
    expect(document.activeElement).toBe(about);
    expect(about.getAttribute("aria-selected")).toBe("true");
  });

  fireEvent.keyDown(about, { key: "ArrowDown" });
  await waitFor(() => {
    expect(document.activeElement).toBe(appearance);
    expect(appearance.getAttribute("aria-selected")).toBe("true");
  });
});
