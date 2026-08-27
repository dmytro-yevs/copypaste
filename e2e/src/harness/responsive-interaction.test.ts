import { describe, expect, test } from "vitest";

import { retryResponsiveInteraction } from "./responsive-interaction.js";

describe("responsive interaction", () => {
  test("reacquires after a stale desktop branch switches to compact", async () => {
    const states = ["desktop-stale", "compact-back", "compact-menu"] as const;
    let index = 0;
    const interactions: string[] = [];

    await retryResponsiveInteraction({
      acquire: async () => {
        const state = states[index] ?? null;
        if (state === "desktop-stale") {
          index += 1;
          throw new Error("stale element reference: branch was replaced");
        }
        return state;
      },
      interact: async (state) => {
        interactions.push(state);
        index += 1;
        return state === "compact-menu";
      },
      waitUntil: async (attempt) => {
        while (!(await attempt())) {
          // The real harness delegates cadence and timeout to WebdriverIO.
        }
      },
    });

    expect(interactions).toEqual(["compact-back", "compact-menu"]);
  });

  test("does not hide an unrelated driver failure", async () => {
    await expect(
      retryResponsiveInteraction({
        acquire: async () => "desktop",
        interact: async () => {
          throw new Error("invalid session id");
        },
        waitUntil: async (attempt) => {
          await attempt();
        },
      }),
    ).rejects.toThrow("invalid session id");
  });
});
