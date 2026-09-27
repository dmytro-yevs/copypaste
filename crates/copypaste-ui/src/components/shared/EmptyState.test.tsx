import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { EmptyState } from "./EmptyState";

describe("EmptyState", () => {
  it("keeps an error live, puts recovery controls in one row, and reveals details on demand", async () => {
    const retry = vi.fn();
    const { container } = render(
      <EmptyState
        tone="danger"
        title="Service unavailable"
        details="The service did not answer."
        action={{ label: "Try again", onClick: retry }}
        secondary={<button type="button">Open diagnostics</button>}
      />,
    );

    expect(screen.getByRole("alert")).toBeTruthy();
    const actions = container.querySelector("[class*='actions']")!;
    expect(actions.contains(screen.getByRole("button", { name: "Try again" }))).toBe(true);
    expect(actions.contains(screen.getByRole("button", { name: "Open diagnostics" }))).toBe(true);
    expect(screen.queryByText("The service did not answer.")).toBeNull();

    await userEvent.setup().click(screen.getByRole("button", { name: "More information" }));
    expect(await screen.findByText("The service did not answer.")).toBeTruthy();
  });
});
