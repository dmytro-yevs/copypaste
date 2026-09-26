import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import * as platform from "@/lib/platform";
import { SearchField } from "./SearchField";

const styles = readFileSync(
  resolve(process.cwd(), "src/components/shared/SearchField.module.css"),
  "utf8",
);
const controlSurfaceStyles = readFileSync(
  resolve(process.cwd(), "src/components/ui/control-surface.module.css"),
  "utf8",
);
const inputStyles = readFileSync(
  resolve(process.cwd(), "src/components/ui/input.module.css"),
  "utf8",
);
const resetStyles = readFileSync(
  resolve(process.cwd(), "src/styles/reset.css"),
  "utf8",
);
const tokenStyles = readFileSync(
  resolve(process.cwd(), "../../design/dist/css/tokens.base.css"),
  "utf8",
);

function pixelToken(source: string, name: string): number {
  const match = source.match(new RegExp(`--${name}:\\s*(\\d+)px;`));
  if (!match) throw new Error(`Missing ${name} token`);
  return Number(match[1]);
}

describe("SearchField", () => {
  it.each([
    ["macos", "⌘F"],
    ["windows", "Ctrl+F"],
  ])("shows the %s shortcut while retaining its raw accessible name", (platform, label) => {
    window.history.replaceState({}, "", `/?platform=${platform}`);
    const { container, unmount } = render(
      <TooltipProvider>
        <SearchField aria-label="Search" shortcut="CmdOrCtrl+F" value="" onChange={() => {}} />
      </TooltipProvider>,
    );
    const badge = container.querySelector("[data-slot='shortcut-badge']");
    expect(badge?.textContent).toBe(label);
    expect(badge?.getAttribute("aria-label")).toBe("CmdOrCtrl+F");
    unmount();
    window.history.replaceState({}, "", "/");
  });

  it("does not show a keyboard badge for touch Android", () => {
    window.history.replaceState({}, "", "/?platform=android");
    const { container } = render(
      <TooltipProvider>
        <SearchField aria-label="Search" shortcut="CmdOrCtrl+F" value="" onChange={() => {}} />
      </TooltipProvider>,
    );
    expect(container.querySelector("[data-slot='shortcut-badge']")).toBeNull();
    window.history.replaceState({}, "", "/");
  });

  it("does not guess a keyboard badge when native platform detection failed", () => {
    const detected = vi.spyOn(platform, "currentPlatform").mockReturnValue("unknown");
    try {
      const { container } = render(
        <TooltipProvider>
          <SearchField aria-label="Search" shortcut="CmdOrCtrl+F" value="" onChange={() => {}} />
        </TooltipProvider>,
      );
      expect(container.querySelector("[data-slot='shortcut-badge']")).toBeNull();
    } finally {
      detected.mockRestore();
    }
  });

  it("does not invent a shortcut when a caller has none", () => {
    const { container } = render(
      <TooltipProvider>
        <SearchField aria-label="Search" value="" onChange={() => {}} />
      </TooltipProvider>,
    );
    expect(container.querySelector("[data-slot='shortcut-badge']")).toBeNull();
  });

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

  it("uses the tap token for both coarse pointers and the capability fallback", () => {
    const coarseTokens = tokenStyles.match(/@media \(pointer: coarse\)\s*\{\s*:root \{(?<tokens>[\s\S]*?)\}\s*\}/);
    const capabilityTokens = tokenStyles.match(/:root\[data-pointer="coarse"\]\s*\{(?<tokens>[\s\S]*?)\}/);
    const fineTapTarget = pixelToken(tokenStyles, "tap-min");
    const tapTarget = pixelToken(coarseTokens?.groups?.tokens ?? "", "tap-min");
    const capabilityTapTarget = pixelToken(capabilityTokens?.groups?.tokens ?? "", "tap-min");
    const surfaceBorder = pixelToken(tokenStyles, "stroke-1");

    expect(controlSurfaceStyles).toMatch(/border:\s*var\(--stroke-1\) solid/);
    expect(inputStyles).toMatch(/\.embedded\s*\{[\s\S]*?min-block-size:\s*100%/);
    expect(resetStyles).toMatch(/\*,[\s\S]*?box-sizing:\s*border-box/);
    expect(fineTapTarget).toBe(32);
    expect(tapTarget).toBe(44);
    expect(capabilityTapTarget).toBe(44);
    expect(tapTarget - surfaceBorder * 2).toBe(42);
    expect(styles).toMatch(
      /\.input\s*\{[\s\S]*?min-block-size:\s*max\(var\(--control-block-size\), var\(--tap-min\)\);/,
    );
  });
});
