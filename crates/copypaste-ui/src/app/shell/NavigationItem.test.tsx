import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { NavigationItem } from "./NavigationItem";

describe("NavigationItem", () => {
  it("blurs a focused field so a view change cannot leave the IME up", async () => {
    const user = userEvent.setup();
    const onClick = vi.fn();
    render(
      <>
        <input aria-label="Cloud endpoint" />
        <NavigationItem
          icon="library"
          label="Library"
          layout="dock"
          active={false}
          onClick={onClick}
        />
      </>,
    );

    const field = screen.getByRole("textbox", { name: "Cloud endpoint" });
    await user.click(field);
    expect(document.activeElement).toBe(field);

    await user.click(screen.getByRole("button", { name: "Library" }));
    expect(document.activeElement).not.toBe(field);
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
