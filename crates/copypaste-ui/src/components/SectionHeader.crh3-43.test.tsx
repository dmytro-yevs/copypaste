/**
 * CopyPaste-crh3.43 — SectionHeader colour: macOS must use text-ide-dim
 * (PARITY-SPEC §3) not text-ide-faint.
 *
 * Root cause: bdac.89 set the default to faint=true based on a stale
 * cross-reference comment that incorrectly claimed Android used c.faint.
 * Android Components.kt:544 actually uses c.dim. crh3.43 corrects the default
 * to faint=false → text-ide-dim.
 */
import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { SectionHeader } from "./SectionHeader";

describe("SectionHeader crh3.43 — PARITY-SPEC §3 colour token", () => {
  it("renders text-ide-dim by default (spec-compliant, matches Android c.dim)", () => {
    const { container } = render(<SectionHeader label="Devices" />);
    const label = container.querySelector("[class*='text-ide-']");
    expect(label).not.toBeNull();
    expect(label!.className).toContain("text-ide-dim");
    expect(label!.className).not.toContain("text-ide-faint");
  });

  it("renders text-ide-faint when faint={true} is explicitly passed", () => {
    // Explicit override is still supported for non-spec decorative uses.
    const { container } = render(<SectionHeader label="Light label" faint />);
    const label = container.querySelector("[class*='text-ide-']");
    expect(label).not.toBeNull();
    expect(label!.className).toContain("text-ide-faint");
  });

  it("renders the label text", () => {
    const { container } = render(<SectionHeader label="Paired devices" />);
    expect(container.textContent).toContain("Paired devices");
  });
});
