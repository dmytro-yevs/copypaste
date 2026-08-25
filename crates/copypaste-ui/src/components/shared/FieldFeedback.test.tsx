import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { FieldFeedback } from "./FieldFeedback";

describe("FieldFeedback", () => {
  it("uses assertive error and polite progress semantics", () => {
    const { rerender } = render(
      <FieldFeedback state="error">Couldn’t save.</FieldFeedback>,
    );

    const error = screen.getByRole("alert");
    expect(error.getAttribute("aria-live")).toBe("assertive");
    expect(error.getAttribute("data-state")).toBe("error");
    expect(error.querySelector('[aria-hidden="true"]')).toBeTruthy();

    rerender(<FieldFeedback state="pending">Saving…</FieldFeedback>);
    const pending = screen.getByRole("status");
    expect(pending.getAttribute("aria-live")).toBe("polite");
    expect(pending.getAttribute("data-state")).toBe("pending");
  });

  it("keeps neutral help quiet unless the caller requests an announcement", () => {
    const { rerender } = render(
      <FieldFeedback state="neutral">Use the default shortcut.</FieldFeedback>,
    );

    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.getByText("Use the default shortcut.")).toBeTruthy();

    rerender(
      <FieldFeedback state="neutral" announce>
        Identifier normalized.
      </FieldFeedback>,
    );
    expect(screen.getByRole("status").textContent).toContain(
      "Identifier normalized.",
    );
  });
});
