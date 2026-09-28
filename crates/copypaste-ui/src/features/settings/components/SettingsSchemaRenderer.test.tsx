import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui";
import { SettingsSchemaRenderer } from "./SettingsSchemaRenderer";

const definition = { id: "test:limit", section: "privacy", kind: "choice", title: "settings.service.historyLimit.title" } as const;
function renderChoice(value: number) {
  return <TooltipProvider><SettingsSchemaRenderer groups={[{ id: "test", title: "Retention", fields: [{
    kind: "choice", definition, value: String(value), options: [{ value: "50", label: "50" }, { value: "100", label: "100" }],
    validation: { min: 100, message: "Choose at least 100 items." }, note: "Current limit",
    onChange: vi.fn(),
  }] }]} /></TooltipProvider>;
}

describe("SettingsSchemaRenderer", () => {
  it("connects validation and note feedback to its control", () => {
    const { rerender } = render(renderChoice(50));
    const select = screen.getByRole("combobox");
    const error = screen.getByRole("alert");
    expect(select.getAttribute("aria-invalid")).toBe("true");
    expect(select.getAttribute("aria-errormessage")).toBe(error.id);
    expect(select.getAttribute("aria-describedby")).toBeTruthy();
    rerender(renderChoice(100));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByRole("combobox").getAttribute("aria-invalid")).toBeNull();
  });
});
