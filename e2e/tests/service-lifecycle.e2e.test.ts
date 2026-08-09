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
import { existsSync, lstatSync, readdirSync } from "node:fs";
import path from "node:path";

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
let realDataBefore: Record<string, string> = {};

function realWindowsDataDir(): string {
  const appData = process.env.APPDATA;
  if (!appData) throw new Error("APPDATA is unavailable in the Windows E2E session");
  // directories 5.0.1 appends this ProjectDirs suffix after asking
  // SHGetKnownFolderPath for FOLDERID_RoamingAppData.
  return path.join(appData, "copypaste", "CopyPaste", "data");
}

function snapshotTree(root: string): Record<string, string> {
  if (!existsSync(root)) return {};
  return Object.fromEntries(
    (readdirSync(root, { recursive: true }) as string[])
      .sort()
      .map((entry) => {
        const stat = lstatSync(path.join(root, entry));
        return [entry, `${stat.isDirectory() ? "d" : "f"}:${stat.size}:${stat.mtimeMs}`];
      }),
  );
}

beforeAll(async () => {
  if (process.platform === "win32") {
    realDataBefore = snapshotTree(realWindowsDataDir());
  }
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

  test("the inherited data directory contains every service-owned file", async () => {
    const entries = readdirSync(app.daemon.dataHome, { recursive: true }) as string[];
    const basenames = entries.map((entry) => path.basename(entry));

    expect(basenames).toContain("copypaste-v2.db");
    expect(basenames).toContain(
      process.platform === "win32" ? "device_secret.dpapi" : "device_secret.key",
    );
    expect(basenames.some((entry) => entry.startsWith("app.") && entry.endsWith(".log"))).toBe(
      true,
    );
    expect(
      basenames.some((entry) => entry.startsWith("daemon.") && entry.endsWith(".log")),
    ).toBe(true);
    if (process.platform === "win32") {
      expect(snapshotTree(realWindowsDataDir())).toEqual(realDataBefore);
    }
  });
});
