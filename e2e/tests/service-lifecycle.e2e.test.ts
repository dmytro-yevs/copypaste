/**
 * What the app does when the background service is not there.
 *
 * ADR-0004 says the app owns the service's lifetime, so the screen must *offer
 * to start it* — the failure this pins is the one where a clipboard manager
 * tells a user to open a terminal, or worse, renders an empty history that says
 * "Nothing copied yet" when the truth is "nothing is listening" (bdac.2).
 *
 * The button is pressed for real: `service::locate` resolves the daemon beside
 * the app's own executable, which in a `target/debug` tree is exactly the
 * binary this harness would have started itself. So this file asserts the offer
 * *works*, not merely that it is rendered.
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import {
  accessibleSurface,
  expectNoFilesystemPath,
  expectNoRawError,
  outerHtml,
} from "../src/harness/leaks.js";
import { clickButton, visibleText, waitForText } from "../src/harness/ui.js";

/** Nothing on this screen may send the user to a shell. */
const SHELL_WORDS = [
  "terminal",
  "sudo",
  "launchctl",
  "systemctl",
  "command line",
  "run the following",
];

let app: App;

beforeAll(async () => {
  // No seed: the offline *screen* only replaces the list when there is nothing
  // else to show — a poll that fails with 200 rows on screen keeps the rows and
  // raises a banner instead, which is a different (and deliberate) behaviour.
  app = await startApp();
  await waitForText(app.browser, "Nothing copied yet");

  // The process goes; the data directory stays, so the service the app starts
  // for itself finds the same database and the same socket path.
  await app.daemon.kill();
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

describe("the service-not-running screen", () => {
  test("says what is wrong and offers to fix it", async () => {
    await waitForText(app.browser, "The clipboard service isn't running", 60_000);

    const text = await visibleText(app.browser);
    expect(text).toContain("background service");

    const start = await app.browser.$('button=Start the service');
    expect(await start.isDisplayed()).toBe(true);
    const box = await start.getSize();
    expect(box.height).toBeGreaterThan(20);
    expect(box.width).toBeGreaterThan(40);
  });

  test("never tells anyone to open a terminal, and names no path", async () => {
    const surface = await accessibleSurface(app.browser);
    for (const word of SHELL_WORDS) {
      expect(surface.toLowerCase(), word).not.toContain(word);
    }
    expectNoFilesystemPath(surface, app.daemon.dataHome);
    expectNoRawError(await outerHtml(app.browser));
  });

  test("keeps the recovery action available after a failed poll", async () => {
    const start = await app.browser.$('button=Start the service');
    expect(await start.isEnabled()).toBe(true);
  });
});

describe("starting it from the screen", () => {
  test("the button really starts the service", async () => {
    await clickButton(app.browser, "Start the service");

    // The CLI is the independent witness: it dials the same socket the app
    // does, so a zero exit means a real daemon is really listening.
    await app.browser.waitUntil(
      async () => (await app.daemon.cli(["--json", "status"])).exitCode === 0,
      {
        timeout: 60_000,
        interval: 500,
        timeoutMsg: "pressing Start never produced a reachable service",
      },
    );

    await waitForText(app.browser, "Nothing copied yet", 60_000);
  });

  test("the recovered window picks up new clippings", async () => {
    await app.daemon.add("arrived after the service came back");
    await waitForText(app.browser, "arrived after the service came back", 30_000);
  });
});
