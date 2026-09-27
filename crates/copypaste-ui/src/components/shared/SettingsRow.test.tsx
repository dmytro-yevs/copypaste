import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { SettingsRow } from "./SettingsRow";

describe("SettingsRow", () => {
  it("opens optional guidance from its keyboard-accessible help control", async () => {
    const user = userEvent.setup();

    render(
      <SettingsRow title="Allow screenshots" help="Captured windows can reveal clipboard content.">
        <button type="button">Change setting</button>
      </SettingsRow>,
    );

    const help = screen.getByRole("button", { name: "More about Allow screenshots" });
    help.focus();
    await user.keyboard("{Enter}");

    expect(screen.getByText("Captured windows can reveal clipboard content.")).toBeTruthy();
  });

  it("marks the actual control for settings search", () => {
    render(
      <SettingsRow title="Allow screenshots" help="Captured windows can reveal clipboard content.">
        <button type="button">Change setting</button>
      </SettingsRow>,
    );

    expect(screen.getByRole("button", { name: "Change setting" }).closest("[data-settings-control]")).toBeTruthy();
  });
});
