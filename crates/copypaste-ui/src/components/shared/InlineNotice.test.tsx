import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { InlineNotice } from "./InlineNotice";

describe("InlineNotice", () => {
  it("keeps the alert semantics and exposes its action", async () => {
    const user = userEvent.setup();
    const retry = vi.fn();

    render(
      <InlineNotice
        role="alert"
        tone="danger"
        icon="alert"
        action={<button onClick={retry}>Try again</button>}
      >
        Could not read the catalogue.
      </InlineNotice>,
    );

    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("Could not read the catalogue.");
    expect(alert.getAttribute("aria-live")).toBeNull();

    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(retry).toHaveBeenCalledOnce();
  });

  it("keeps the existing opt-in polite live region", () => {
    render(<InlineNotice live>Clipboard settings changed.</InlineNotice>);

    expect(
      screen
        .getByText("Clipboard settings changed.")
        .closest("[aria-live]")
        ?.getAttribute("aria-live"),
    ).toBe("polite");
  });
});
