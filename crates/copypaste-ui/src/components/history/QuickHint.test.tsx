/**
 * ADR-0001, consequence 1: the app never synthesises ⌘V, so the hint saying the
 * user has to press it is load-bearing copy rather than decoration. The e2e
 * suite asserts the same sentence against a real WebView; this asserts it
 * survives `Trans`, which is the part that could silently render the key name
 * or drop the `<kbd>` runs.
 */
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { QuickHint } from "@/components/history/QuickHint";

describe("the quick-copy hint", () => {
  it("tells the user they still have to paste", () => {
    const { container } = render(<QuickHint searching={false} />);
    expect(container.textContent).toContain(
      "Copying puts the item on the clipboard — press ⌘V yourself to paste",
    );
    expect(screen.getByText("⌘V").tagName).toBe("KBD");
  });

  it("advertises ⌘1–⌘9 only when a search is not filtering the list (§3.5.3)", () => {
    const { container, rerender } = render(<QuickHint searching={false} />);
    expect(container.textContent).toContain("⌘1–⌘9 copy and close");

    rerender(<QuickHint searching />);
    expect(container.textContent).not.toContain("copy and close");
    expect(container.textContent).toContain("↑↓ move");
  });
});
