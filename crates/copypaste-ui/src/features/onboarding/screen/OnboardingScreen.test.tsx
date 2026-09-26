import { afterEach, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import { TooltipProvider } from "@/components/ui";
import { useUi } from "@/store/ui";
import { OnboardingScreen } from "./OnboardingScreen";

afterEach(() => {
  useUi.setState({ onboardingOpen: false, view: "history" });
});

it("uses step buttons instead of incomplete tab semantics", () => {
  render(
    <TooltipProvider>
      <OnboardingScreen />
    </TooltipProvider>,
  );

  const steps = screen.getByRole("navigation", { name: "Onboarding steps" });
  const firstStep = screen.getByRole("button", { name: "Step 1 of 3" });
  expect(steps.contains(firstStep)).toBe(true);
  expect(screen.queryByRole("tablist")).toBeNull();
  expect(firstStep.getAttribute("aria-current")).toBe("step");

  fireEvent.click(screen.getByRole("button", { name: "Step 2 of 3" }));
  expect(firstStep.getAttribute("aria-current")).toBeNull();
  expect(screen.getByRole("button", { name: "Step 2 of 3" }).getAttribute("aria-current")).toBe("step");
});

it("labels the desktop capture step by its actual navigation action", () => {
  render(
    <TooltipProvider>
      <OnboardingScreen />
    </TooltipProvider>,
  );

  fireEvent.click(screen.getByRole("button", { name: "Set up capture" }));
  expect(screen.getByRole("button", { name: "Continue" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Enable capture" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  expect(screen.getByRole("heading", { name: "Bring your devices together." })).toBeTruthy();
});

it("starts every changed step at its heading after the user scrolls", () => {
  const { container } = render(
    <TooltipProvider>
      <OnboardingScreen />
    </TooltipProvider>,
  );
  const viewport = container.querySelector<HTMLElement>("[data-onboarding-scroll]");
  expect(viewport).not.toBeNull();

  for (const [step, title] of [
    [2, "Keep new copies within reach."],
    [3, "Bring your devices together."],
    [1, "Your clipboard finally remembers."],
  ] as const) {
    viewport!.scrollTop = 302;
    fireEvent.click(screen.getByRole("button", { name: `Step ${step} of 3` }));
    expect(viewport!.scrollTop).toBe(0);
    expect(screen.getByRole("heading", { name: title })).toBe(document.activeElement);
  }
});
