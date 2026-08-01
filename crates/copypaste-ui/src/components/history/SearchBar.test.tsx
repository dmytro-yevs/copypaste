import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";

import { SearchBar } from "@/components/history/SearchBar";
import { DEFAULT_VIEW } from "@/lib/view";

describe("History search toolbar", () => {
  it("uses the same pointer-aware height for search and select controls", () => {
    render(
      <SearchBar
        value=""
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

    const controls = [
      screen.getByRole("searchbox", { name: "Search clipboard history" }),
      screen.getByRole("combobox", { name: "Filter by kind" }),
      screen.getByRole("combobox", { name: "Sort order" }),
    ];
    const toolbar = screen
      .getByRole("searchbox", { name: "Search clipboard history" })
      .closest<HTMLElement>('[data-slot="history-toolbar"]')!;

    expect(toolbar.className).toContain("bg-transparent");
    expect(toolbar.className).not.toContain("chrome");

    for (const control of controls) {
      expect(control.className).toContain("h-9");
      expect(control.className).toContain("min-h-[var(--tap-min)]");
      if (control instanceof HTMLSelectElement) {
        expect(control.className).toContain("appearance-none");
        expect(control.className).toContain("pr-10");
      }
    }
  });
});
