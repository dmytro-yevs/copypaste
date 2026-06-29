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
import type { UIPrefs } from "../../../store";

// i3a7: dead v3 appearance fields (density/skin/palette) removed — UIPrefs v4 is
// the two-axis shape (theme + accent + the booleans below).
const defaultPrefs: UIPrefs = {
  previewLinesApp: 1,
  previewLinesPopup: 1,
  previewSize: 28,
  maskSensitive: true,
  imageMaxHeight: 40,
  playSoundOnCopy: true,
  notifyOnCopy: true,
  translucency: true,
  theme: "dark",
  accent: "indigo",
  historyDisplayLimit: 1000,
  showSensitiveWarnings: true,
  sortByDevice: false,
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
