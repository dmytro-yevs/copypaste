import { Check, Circle } from "lucide-react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { Stepper } from "@/components/ui/stepper";

describe("Stepper", () => {
  it("renders the shared accessible step list contract", () => {
    render(
      <Stepper
        label="Setup steps"
        items={[
          { id: "first", label: "Install", stateLabel: "Done", icon: Check, done: true },
          { id: "second", label: "Authorize", stateLabel: "Next", icon: Circle, current: true },
        ]}
      />,
    );

    expect(screen.getByRole("list", { name: "Setup steps" })).toBeTruthy();
    expect(document.querySelector('[data-step="first"]')?.textContent).toContain("Done");
    expect(document.querySelector('[data-step="second"]')?.textContent).toContain("Next");
  });
});
