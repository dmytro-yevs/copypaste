import { describe, expect, it } from "vitest";
import { screen } from "@testing-library/react";

import App from "@/App";
import { withUser } from "@/test/harness";

describe("desktop shell", () => {
  it("exposes the current screen as the document's main region", () => {
    withUser(<App />);

    expect(screen.getByRole("main")).toBeTruthy();
  });

  it("uses a sidebar rather than the Android tab bar", () => {
    withUser(<App />);

    expect(screen.getByRole("navigation", { name: "Primary" }).className).toContain(
      "border-r",
    );
  });

  it("loads an inactive screen when its navigation item is selected", async () => {
    const { user } = withUser(<App />);

    await user.click(screen.getByRole("button", { name: "Devices" }));

    expect(await screen.findByRole("heading", { name: "Devices" })).not.toBeNull();
  });
});
