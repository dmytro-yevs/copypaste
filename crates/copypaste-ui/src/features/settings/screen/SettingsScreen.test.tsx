import { afterEach, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import { TooltipProvider } from "@/components/ui";
import { useUi } from "@/store/ui";
import { SettingsScreen } from "./SettingsScreen";

vi.mock("@/hooks/useViewportMetrics", () => ({
  useViewportMetrics: () => ({
    width: 390,
    height: 800,
    pointer: "coarse",
    sizeClass: "compact",
  }),
  useObservedElementSize: () => ({
    ref: () => {},
    width: 390,
    height: 800,
  }),
}));

vi.mock("@/features/settings/patterns/settingsTabs", () => ({
  renderPreferenceSection: (section: string) => (
    <div data-testid={`settings-section-${section}`} />
  ),
}));

afterEach(() => {
  vi.restoreAllMocks();
  useUi.setState({ settingsTab: null });
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

  const menu = screen.getByRole("navigation", {
    name: "Preference sections",
  });
  const viewport = menu.parentElement?.parentElement;
  expect(viewport).toBeInstanceOf(HTMLDivElement);
  if (!(viewport instanceof HTMLDivElement)) return;

  viewport.scrollTop = 240;
  fireEvent.click(screen.getByRole("button", { name: /^Appearance/ }));

  expect(await screen.findByTestId("settings-section-appearance")).toBeTruthy();
  expect(viewport.scrollTop).toBe(0);

  viewport.scrollTop = 240;
  fireEvent.click(screen.getByRole("button", { name: "Back to Preferences" }));

  expect(await screen.findByRole("navigation", {
    name: "Preference sections",
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
