import { createRef } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SearchBar } from "@/components/history/SearchBar";
import { DEFAULT_VIEW } from "@/lib/view";
import * as platform from "@/lib/platform";

afterEach(() => vi.restoreAllMocks());
function toolbar() {
  return screen
    .getByLabelText(/clipboard history/i)
    .closest<HTMLElement>('[data-slot="history-toolbar"]')!;
}

function renderToolbar(value = "") {
  return render(
    <SearchBar
      value={value}
      onChange={vi.fn()}
      onEnterList={vi.fn()}
      inputRef={createRef<HTMLInputElement>()}
      filtered={false}
      visible={0}
      total={0}
      view={DEFAULT_VIEW}
      onViewChange={vi.fn()}
      origins={[]}
      displayLimit={null}
      selecting={false}
      onToggleSelecting={vi.fn()}
    />,
  );
}

describe("History search toolbar", () => {
  it("keeps desktop search visible beside the pointer-sized controls", () => {
    vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(false);
    renderToolbar();

    const controls = [
      screen.getByRole("searchbox", { name: "Search clipboard history" }),
      screen.getByRole("combobox", { name: "Filter by kind" }),
      screen.getByRole("combobox", { name: "Sort order" }),
    ];
    const toolbarElement = toolbar();

    expect(toolbarElement.className).toContain("flex-nowrap");
    expect(toolbarElement.className).toContain("bg-transparent");
    expect(toolbarElement.className).not.toContain("chrome");

    for (const control of controls) {
      expect(control.className).toContain("min-h-[var(--tap-min)]");
      expect(control.getAttribute("data-slot")).toBe(
        control === controls[0] ? "input" : "select-trigger",
      );
    }
  });

  it("replaces the Android control row with a full-width search field", async () => {
    vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(true);
    const user = userEvent.setup();
    renderToolbar();

    expect(screen.queryByRole("searchbox")).toBeNull();
    expect(
      screen.getByRole("combobox", { name: "Filter by kind" }),
    ).toBeTruthy();
    expect(toolbar().className).toContain("flex-nowrap");

    await user.click(
      screen.getByRole("button", { name: "Search clipboard history" }),
    );

    const search = screen.getByRole("searchbox", {
      name: "Search clipboard history",
    });
    expect(search).toBeTruthy();
    expect(
      screen.queryByRole("combobox", { name: "Filter by kind" }),
    ).toBeNull();
    expect(toolbar().getAttribute("data-search-open")).toBe("true");
    expect(search.parentElement?.className).toContain("flex-1");
  });

  it("keeps an Android query until it is explicitly cleared", async () => {
    vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(true);
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(
      <SearchBar
        value="kept query"
        onChange={onChange}
        onEnterList={vi.fn()}
        inputRef={createRef<HTMLInputElement>()}
        filtered
        visible={1}
        total={4}
        view={DEFAULT_VIEW}
        onViewChange={vi.fn()}
        origins={[]}
        displayLimit={null}
        selecting={false}
        onToggleSelecting={vi.fn()}
      />,
    );

    await user.click(
      screen.getByRole("button", { name: "Search clipboard history" }),
    );
    await user.click(screen.getByRole("button", { name: "Clear search" }));
    expect(onChange).toHaveBeenCalledWith("");
  });
});
