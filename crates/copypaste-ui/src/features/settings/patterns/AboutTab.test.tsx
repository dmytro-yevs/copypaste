import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { t } from "@/i18n";
import { SETTINGS_SEARCH_ITEMS } from "@/features/settings/model/settingsSearchIndex";
import { useUi } from "@/store/ui";
import { AboutTab } from "./AboutTab";

vi.mock("@/features/settings/components/UpdateRow", () => ({
  UpdateRow: () => (
    <div data-settings-search-target="row:App updates" />
  ),
}));

vi.mock("@/hooks/useStatus", () => ({
  statusService: {},
  useStatus: () => ({ data: undefined, error: null }),
}));

vi.mock("@/lib/appVersion", () => ({
  appVersion: () => Promise.resolve("2.0.0-test"),
}));

beforeEach(() => {
  vi.stubGlobal("__COPYPASTE_APP_VERSION__", "2.0.0-test");
});

afterEach(() => {
  vi.unstubAllGlobals();
  useUi.setState({ onboardingOpen: false });
});

describe("About settings", () => {
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

    fireEvent.click(screen.getByRole("button", { name: "Open welcome" }));

    expect(useUi.getState().onboardingOpen).toBe(true);
  });
});
