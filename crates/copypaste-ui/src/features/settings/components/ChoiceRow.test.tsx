import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { ChoiceRow } from "./ChoiceRow";

const choices = [
  { value: 50, unit: "items", count: 50 },
  { value: 100, unit: "items", count: 100 },
] as const;

function row(value: number) {
  return (
    <TooltipProvider>
      <ChoiceRow
        title="History limit"
        description="Maximum stored items."
        icon="storage"
        choices={choices}
        value={value}
        validation={{ min: 100, message: "Choose at least 100 items." }}
        onChange={vi.fn()}
      />
    </TooltipProvider>
  );
}

describe("ChoiceRow feedback", () => {
  it("connects invalid controls to the shared error feedback", () => {
    const { rerender } = render(row(50));

    const select = screen.getByRole("combobox");
    const error = screen.getByRole("alert");
    expect(error.textContent).toContain("Choose at least 100 items.");
    expect(select.getAttribute("aria-invalid")).toBe("true");
    expect(select.getAttribute("aria-errormessage")).toBe(error.id);

    rerender(row(100));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByRole("combobox").getAttribute("aria-invalid")).toBeNull();
    expect(
      screen.getByRole("combobox").getAttribute("aria-errormessage"),
    ).toBeNull();
  });
});
