import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { NativeSelect } from "@/components/ui/native-select";

describe("NativeSelect", () => {
  it("keeps the platform picker and gives its chevron a clear inset", () => {
    render(
      <NativeSelect aria-label="Choice">
        <option>One</option>
      </NativeSelect>,
    );

    const select = screen.getByRole("combobox", { name: "Choice" });
    expect(select.className).toContain("appearance-none");
    expect(select.className).toContain("pr-10");
    expect(screen.getByTestId("native-select-chevron").getAttribute("class")).toContain("right-3");
  });
});
