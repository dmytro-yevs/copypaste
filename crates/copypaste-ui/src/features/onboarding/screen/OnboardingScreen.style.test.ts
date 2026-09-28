import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const screenCss = readFileSync(
  resolve(process.cwd(), "src/features/onboarding/screen/OnboardingScreen.module.css"),
  "utf8",
);
const screenSource = readFileSync(
  resolve(process.cwd(), "src/features/onboarding/screen/OnboardingScreen.tsx"),
  "utf8",
);
describe("Onboarding responsive layout", () => {
  it("styles the current step represented by the pagination buttons", () => {
    expect(screenSource).toContain('aria-current={dotIndex === index ? "step" : undefined}');
    expect(screenCss.match(/\.dot\[aria-current="step"\]::before/g)).toHaveLength(2);
    expect(screenCss).not.toContain("aria-selected");
  });

  it("collapses the split layout at the maintained narrow toolbar breakpoint", () => {
    expect(screenCss).toMatch(
      /@media \(--cp-toolbar\)[\s\S]*grid-template-columns:\s*minmax\(0, 1fr\)/,
    );
    expect(screenCss).not.toMatch(/@media \(--cp-md\)/);
  });

  it("reserves the last mobile row for actions while allowing onboarding content to scroll", () => {
    expect(screenCss).toMatch(
      /@media \(--cp-toolbar\)[\s\S]*--onboarding-content-size:\s*clamp\([^;]*vh[^;]*\)[\s\S]*block-size:\s*100%;[\s\S]*grid-template-rows:\s*auto auto auto auto minmax\(var\(--onboarding-content-size\), 1fr\) auto/,
    );
    expect(screenCss).toMatch(/\.window\s*\{[\s\S]*overflow-y:\s*auto/);
    expect(screenCss).toMatch(/\.content\[data-interactive\]\s*\{[\s\S]*align-items:\s*start/);
    expect(screenCss).not.toMatch(/12\.8125/);
  });
});
