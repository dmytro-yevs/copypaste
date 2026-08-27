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
