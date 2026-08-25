import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Button } from "@/components/ui";
import { StatusCard } from "./StatusCard";

describe("StatusCard", () => {
  it("owns live, busy, copy, indicator, and action semantics", () => {
    render(
      <StatusCard
        status="danger"
        title="Capture failed"
        detail="The clipboard refused the read."
        meta="Last saved one minute ago"
        icon="alert"
        action={<Button>Try again</Button>}
        role="alert"
        live="assertive"
        busy
      />,
    );

    const card = screen.getByRole("alert");
    expect(card.getAttribute("aria-live")).toBe("assertive");
    expect(card.getAttribute("aria-busy")).toBe("true");
    expect(card.getAttribute("data-status")).toBe("danger");
    expect(screen.getByText("The clipboard refused the read.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });
});
