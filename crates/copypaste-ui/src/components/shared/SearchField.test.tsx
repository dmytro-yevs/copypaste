import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { SearchField } from "./SearchField";

describe("SearchField", () => {
  it("keeps its searchbox vocabulary and renders one clear control", () => {
    render(
      <TooltipProvider>
        <SearchField
          aria-label="Search clipboard"
          value="clipboard"
          onChange={() => {}}
          onClear={() => {}}
        />
      </TooltipProvider>,
    );

    expect(screen.getByRole("searchbox", { name: "Search clipboard" })).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Clear search" })).toHaveLength(1);
  });
});
