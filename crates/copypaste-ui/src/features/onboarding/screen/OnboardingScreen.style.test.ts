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

describe("Onboarding responsive layout", () => {
  it("collapses the split layout at the maintained medium breakpoint", () => {
    expect(screenCss).toMatch(
      /@media \(--cp-md\)[\s\S]*grid-template-columns:\s*minmax\(0, 1fr\)/,
    );
    expect(artworkCss).toMatch(/@media \(--cp-md\)/);
  });

  it("does not present the secure device hub as a broken cloud", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/features/onboarding/components/OnboardingArtwork.tsx"),
      "utf8",
    );

    expect(source).toContain('name="devices"');
    expect(source).toContain('name="lock"');
    expect(source).not.toContain('name="cloudOff"');
  });
});
