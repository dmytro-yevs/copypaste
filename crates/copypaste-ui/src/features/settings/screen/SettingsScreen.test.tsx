import { afterEach, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import { TooltipProvider } from "@/components/ui";
import { useUi } from "@/store/ui";
import { SettingsScreen } from "./SettingsScreen";

const viewport = vi.hoisted(() => ({
  width: 390,
  height: 800,
  pointer: "coarse" as const,
  sizeClass: "compact" as const,
}));

vi.mock("@/hooks/useViewportMetrics", () => ({
  useViewportMetrics: () => viewport,
  useObservedElementSize: () => ({
    ref: () => {},
    width: viewport.width,
    height: viewport.height,
  }),
}));

vi.mock("@/features/settings/patterns/settingsTabs", () => ({
  renderPreferenceSection: (section: string) => (
    <div
      data-testid={`settings-section-${section}`}
      data-settings-search-target={
        section === "clipboard" ? "row:Group by device" : undefined
      }
    />
  ),
}));

afterEach(() => {
  vi.restoreAllMocks();
  useUi.setState({ settingsTab: null });
  Object.assign(viewport, {
    width: 390,
    height: 800,
    pointer: "coarse",
    sizeClass: "compact",
  });
});

it("resets compact settings scroll when opening and leaving a detail", async () => {
  vi.spyOn(window.history, "back").mockImplementation(() => {
    window.dispatchEvent(new PopStateEvent("popstate"));
  });
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
  expect(panel.getAttribute("aria-labelledby")).toBe(appearance.id);
  expect(appearance.getAttribute("aria-controls")).toBe(panel.id);

  appearance.focus();
  fireEvent.keyDown(appearance, { key: "ArrowRight" });
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

  fireEvent.keyDown(appearance, { key: "ArrowLeft" });
  await waitFor(() => {
    expect(document.activeElement).toBe(about);
    expect(about.getAttribute("aria-selected")).toBe("true");
  });

  fireEvent.keyDown(about, { key: "ArrowRight" });
  await waitFor(() => {
    expect(document.activeElement).toBe(appearance);
    expect(appearance.getAttribute("aria-selected")).toBe("true");
  });
});
