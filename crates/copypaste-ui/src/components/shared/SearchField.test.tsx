import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { SearchField } from "./SearchField";

const styles = readFileSync(
  resolve(process.cwd(), "src/components/shared/SearchField.module.css"),
  "utf8",
);

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

  it("suppresses the native search cancellation control", () => {
    expect(styles).toMatch(
      /::-webkit-search-cancel-button\s*\{\s*-webkit-appearance:\s*none;\s*appearance:\s*none;\s*display:\s*none;/,
    );
  });
});
