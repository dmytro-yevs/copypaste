/**
 * INV-35 has a UI or it has nothing (CLAUDE.md rule 6): a protection nobody can
 * turn off for a support call gets turned off by uninstalling the app, and one
 * nobody can find protects only the people who went looking.
 *
 * What is asserted here is the *safe* direction — the control starts off, and
 * turning it on says on screen what it costs.
 */
import { afterEach, describe, expect, it } from "vitest";
import { screen } from "@testing-library/react";

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
