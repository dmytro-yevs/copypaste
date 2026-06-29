/**
 * CopyPaste-2hfj.4 — Phase 3: Appearance tab = Theme + Accent + Translucency + Mask sensitive.
 *
 * Guards the new two-axis DisplayTab (§2 STYLEGUIDE):
 *  - Theme segmented control: Light / Dark (no System option)
 *  - Accent picker: 6 swatches — indigo · blue · teal · green · amber · rose
 *  - Translucency toggle
 *  - Mask sensitive toggle
 *  - Removed controls: palette, skin, density, contrast, motionReduced
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { DisplayTab } from "./DisplayTab";
import type { UIPrefs } from "../../../store";

const BASE_PREFS: UIPrefs = {
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

describe("DisplayTab Phase 3 — Appearance controls (2hfj.4)", () => {
  // ── Theme segmented control ───────────────────────────────────────────────

  it("renders a 'Light' and 'Dark' button in the theme segmented control", () => {
    render(<DisplayTab prefs={BASE_PREFS} setPrefs={() => {}} />);
    expect(screen.getByRole("button", { name: "Light" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Dark" })).toBeInTheDocument();
  });

  it("does NOT render a 'System' theme option", () => {
    render(<DisplayTab prefs={BASE_PREFS} setPrefs={() => {}} />);
    expect(screen.queryByRole("button", { name: "System" })).not.toBeInTheDocument();
  });

  it("marks the active theme button as aria-pressed=true", () => {
    const { rerender } = render(
      <DisplayTab prefs={{ ...BASE_PREFS, theme: "dark" }} setPrefs={() => {}} />
    );
    expect(screen.getByRole("button", { name: "Dark" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "false");

    rerender(
      <DisplayTab prefs={{ ...BASE_PREFS, theme: "light" }} setPrefs={() => {}} />
    );
    expect(screen.getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Dark" })).toHaveAttribute("aria-pressed", "false");
  });

  it("calls setPrefs({ theme: 'light' }) when Light is clicked", () => {
    const setPrefs = vi.fn();
    render(<DisplayTab prefs={{ ...BASE_PREFS, theme: "dark" }} setPrefs={setPrefs} />);
    fireEvent.click(screen.getByRole("button", { name: "Light" }));
    expect(setPrefs).toHaveBeenCalledWith({ theme: "light" });
  });

  it("calls setPrefs({ theme: 'dark' }) when Dark is clicked", () => {
    const setPrefs = vi.fn();
    render(<DisplayTab prefs={{ ...BASE_PREFS, theme: "light" }} setPrefs={setPrefs} />);
    fireEvent.click(screen.getByRole("button", { name: "Dark" }));
    expect(setPrefs).toHaveBeenCalledWith({ theme: "dark" });
  });

  // ── Accent swatch picker ──────────────────────────────────────────────────

  it("renders 6 accent swatch buttons with correct labels", () => {
    render(<DisplayTab prefs={BASE_PREFS} setPrefs={() => {}} />);
    const accentIds = ["Indigo", "Blue", "Teal", "Green", "Amber", "Rose"];
    for (const label of accentIds) {
      expect(screen.getByRole("button", { name: label })).toBeInTheDocument();
    }
  });

  it("marks the active accent swatch as aria-pressed=true", () => {
    render(<DisplayTab prefs={{ ...BASE_PREFS, accent: "teal" }} setPrefs={() => {}} />);
    expect(screen.getByRole("button", { name: "Teal" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Indigo" })).toHaveAttribute("aria-pressed", "false");
  });

  it("calls setPrefs({ accent: 'rose' }) when the Rose swatch is clicked", () => {
    const setPrefs = vi.fn();
    render(<DisplayTab prefs={BASE_PREFS} setPrefs={setPrefs} />);
    fireEvent.click(screen.getByRole("button", { name: "Rose" }));
    expect(setPrefs).toHaveBeenCalledWith({ accent: "rose" });
  });

  // ── Optional toggles ──────────────────────────────────────────────────────

  it("renders a Translucency toggle wired to prefs.translucency", () => {
    const setPrefs = vi.fn();
    render(<DisplayTab prefs={{ ...BASE_PREFS, translucency: false }} setPrefs={setPrefs} />);
    // Row label is present
    expect(screen.getByText("Translucency")).toBeInTheDocument();
    // The switch reflects the pref value: aria-checked=false when translucency=false
    const toggle = screen.getByRole("switch", { name: /translucency/i });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    fireEvent.click(toggle);
    expect(setPrefs).toHaveBeenCalledWith({ translucency: true });
  });

  it("renders a Mask sensitive data toggle wired to prefs.maskSensitive", () => {
    const setPrefs = vi.fn();
    render(<DisplayTab prefs={{ ...BASE_PREFS, maskSensitive: true }} setPrefs={setPrefs} />);
    expect(screen.getByText("Mask sensitive data")).toBeInTheDocument();
    const toggle = screen.getByRole("switch", { name: /mask sensitive/i });
    expect(toggle).toHaveAttribute("aria-checked", "true");
    fireEvent.click(toggle);
    expect(setPrefs).toHaveBeenCalledWith({ maskSensitive: false });
  });

  // ── Removed controls must NOT appear ─────────────────────────────────────

  it("does NOT render a Color palette or palette picker control", () => {
    render(<DisplayTab prefs={BASE_PREFS} setPrefs={() => {}} />);
    expect(screen.queryByTestId("palette-picker")).not.toBeInTheDocument();
    expect(screen.queryByText("Color palette")).not.toBeInTheDocument();
  });

  it("does NOT render a Visual style / skin picker control", () => {
    render(<DisplayTab prefs={BASE_PREFS} setPrefs={() => {}} />);
    expect(screen.queryByTestId("skin-picker")).not.toBeInTheDocument();
    expect(screen.queryByText("Visual style")).not.toBeInTheDocument();
  });

  it("does NOT render a Row density control", () => {
    render(<DisplayTab prefs={BASE_PREFS} setPrefs={() => {}} />);
    expect(screen.queryByText("Row density")).not.toBeInTheDocument();
  });

  it("does NOT render a Reduce motion control", () => {
    render(<DisplayTab prefs={BASE_PREFS} setPrefs={() => {}} />);
    expect(screen.queryByText("Reduce motion")).not.toBeInTheDocument();
  });
});
