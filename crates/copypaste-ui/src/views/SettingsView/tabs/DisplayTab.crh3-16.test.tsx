/**
 * CopyPaste-crh3.16 — "Warn before revealing sensitive" missing "items".
 *
 * Guards that the setting label is a complete phrase:
 *   "Warn before revealing sensitive items"
 * (not the dangling "Warn before revealing sensitive" that shipped before).
 */
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { DisplayTab } from "./DisplayTab";

const defaultPrefs = {
  theme: "dark" as const,
  density: "comfortable" as const,
  skin: "classic" as const,
  palette: "graphite-mist" as const,
};

describe("DisplayTab crh3.16 — sensitive-warning label completeness", () => {
  it("renders the complete label 'Warn before revealing sensitive items'", () => {
    render(
      <DisplayTab prefs={defaultPrefs} setPrefs={() => {}} />
    );
    expect(
      screen.getByText("Warn before revealing sensitive items")
    ).toBeInTheDocument();
  });

  it("does NOT render the truncated 'Warn before revealing sensitive' label", () => {
    render(
      <DisplayTab prefs={defaultPrefs} setPrefs={() => {}} />
    );
    // The exact dangling string should not match any standalone element
    const truncated = screen.queryByText(
      (content) =>
        content === "Warn before revealing sensitive"
    );
    expect(truncated).not.toBeInTheDocument();
  });
});
