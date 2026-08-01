/**
 * INV-35 has a UI or it has nothing (CLAUDE.md rule 6): a protection nobody can
 * turn off for a support call gets turned off by uninstalling the app, and one
 * nobody can find protects only the people who went looking.
 *
 * What is asserted here is the *safe* direction — the control starts off, and
 * turning it on says on screen what it costs.
 */
import { afterEach, describe, expect, it } from "vitest";
import { fireEvent, screen } from "@testing-library/react";

import { ListTab } from "@/components/settings/ListTab";
import { en } from "@/i18n/en";
import { withUser } from "@/test/harness";
import { usePrefs } from "@/store/prefs";

afterEach(() => {
  usePrefs.getState().reset();
  window.localStorage.clear();
});

const COPY = en.settings.list.allowScreenshots;

describe("allow screenshots", () => {
  it("starts off, so a first run is protected without anyone choosing", () => {
    withUser(<ListTab />);
    expect(screen.getByLabelText(COPY.title).getAttribute("aria-checked")).toBe(
      "false",
    );
    expect(screen.queryByText(COPY.warning)).toBeNull();
  });

  it("records the choice and says what it costs while it is on", async () => {
    const { user } = withUser(<ListTab />);
    await user.click(screen.getByLabelText(COPY.title));

    expect(usePrefs.getState().allowScreenshots).toBe(true);
    expect(screen.getByText(COPY.warning)).toBeTruthy();
  });
});

describe("display preferences", () => {
  it("persists group-by-device from the same control the toolbar mirrors", async () => {
    const { user } = withUser(<ListTab />);
    const control = screen.getByLabelText(
      en.settings.list.groupByDevice.title,
    );
    expect(control.getAttribute("aria-checked")).toBe("false");
    await user.click(control);
    expect(usePrefs.getState().sortByDevice).toBe(true);
  });

  it("offers bounded popup and history display controls", () => {
    withUser(<ListTab />);

    fireEvent.keyDown(
      screen.getByRole("slider", {
        name: en.settings.list.popupPreviewLines.title,
      }),
      { key: "End" },
    );
    fireEvent.keyDown(
      screen.getByRole("slider", {
        name: en.settings.list.historyDisplayLimit.title,
      }),
      { key: "End" },
    );

    expect(usePrefs.getState().previewLinesPopup).toBe(6);
    expect(usePrefs.getState().historyDisplayLimit).toBe(100_000);
    expect(screen.getByText("Unlimited")).toBeTruthy();
  });
});
