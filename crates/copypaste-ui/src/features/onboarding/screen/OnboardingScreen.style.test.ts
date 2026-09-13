import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const screenCss = readFileSync(
  resolve(process.cwd(), "src/features/onboarding/screen/OnboardingScreen.module.css"),
  "utf8",
);
const artworkCss = readFileSync(
  resolve(process.cwd(), "src/features/onboarding/components/OnboardingArtwork.module.css"),
  "utf8",
);
const artworkSource = readFileSync(
  resolve(process.cwd(), "src/features/onboarding/components/OnboardingArtwork.tsx"),
  "utf8",
);

describe("Onboarding responsive layout", () => {
  it("collapses the split layout at the maintained narrow toolbar breakpoint", () => {
    expect(screenCss).toMatch(
      /@media \(--cp-toolbar\)[\s\S]*grid-template-columns:\s*minmax\(0, 1fr\)/,
    );
    expect(artworkCss).toMatch(/@media \(--cp-toolbar\)/);
    expect(screenCss).not.toMatch(/@media \(--cp-md\)/);
  });

  it("does not present the secure device hub as a broken cloud", () => {
    expect(artworkSource).toContain('name="devices"');
    expect(artworkSource).toContain('name="lock"');
    expect(artworkSource).not.toContain('name="cloudOff"');
  });

  it("sizes marketing artwork glyphs from scene tokens instead of toolbar icons", () => {
    expect(artworkCss).toMatch(
      /\.captureCore \.sceneGlyph,\s*\.networkHub \.sceneGlyph\s*\{[\s\S]*font-size:\s*calc\(var\(--icon-lg\) \* 3\)/,
    );
    expect(artworkCss).toMatch(
      /\.captureCardIcon \.cardGlyph,\s*\.deviceNode \.satelliteGlyph\s*\{[\s\S]*font-size:\s*var\(--s-8\)/,
    );
    expect(artworkCss).not.toMatch(/\.captureCardIcon svg \{\s*inline-size: var\(--icon-sm\)/);
    expect(artworkCss).not.toMatch(/\.deviceNode svg \{\s*inline-size: var\(--icon-md\)/);
    expect(artworkSource).toContain("styles.sceneGlyph");
    expect(artworkSource).toContain("styles.cardGlyph");
    expect(artworkSource).toContain("styles.satelliteGlyph");
  });
});
