import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Tabs } from "./tabs";

describe("data-driven Tabs", () => {
  it("forwards selection and connects each tab to its panel", () => {
    const onValueChange = vi.fn();
    render(
      <Tabs
        value="general"
        onValueChange={onValueChange}
        listProps={{ "aria-label": "Settings sections" }}
        items={[
          { value: "general", label: "General", content: "General settings" },
          { value: "privacy", label: "Privacy", content: "Privacy settings" },
        ]}
      />,
    );

    const general = screen.getByRole("tab", { name: "General" });
    const privacy = screen.getByRole("tab", { name: "Privacy" });
    expect(screen.getByRole("tablist", { name: "Settings sections" })).toBeTruthy();
    expect(general.getAttribute("aria-controls")).toBe(screen.getByRole("tabpanel").id);
    fireEvent.mouseDown(privacy, { button: 0 });
    expect(onValueChange).toHaveBeenCalledWith("privacy");
  });
});
