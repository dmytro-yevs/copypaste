import { execa } from "execa";
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import {
  accessibleSurface,
  expectNoFilesystemPath,
} from "../src/harness/leaks.js";
import {
  clickButton,
  gotoView,
  visibleText,
  waitForRows,
  waitForText,
} from "../src/harness/ui.js";

let app: App;

beforeAll(async () => {
  if (process.platform !== "win32") {
    throw new Error("the Windows native surface suite ran on a non-Windows host");
  }
  if (process.env.COPYPASTE_WINDOWS_SHELL_E2E !== "1") {
    throw new Error("COPYPASTE_WINDOWS_SHELL_E2E=1 is required for registry changes");
  }
  app = await startApp({ seed: ["a Windows surface fixture"] });
  await waitForRows(app.browser, 1);
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

async function openTab(label: string): Promise<void> {
  await gotoView(app.browser, "Settings");
  const tab = await app.browser.$(`[role="tab"]=${label}`);
  await tab.waitForClickable({ timeout: 15_000 });
  await tab.click();
  await app.browser.waitUntil(
    async () => (await tab.getAttribute("aria-selected")) === "true",
    { timeout: 10_000, timeoutMsg: `the ${label} tab never became selected` },
  );
}

async function setClipboard(value: string): Promise<void> {
  await execa(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      "Set-Clipboard -Value $env:COPYPASTE_E2E_CLIPBOARD",
    ],
    {
      env: { ...process.env, COPYPASTE_E2E_CLIPBOARD: value },
      timeout: 20_000,
    },
  );
}

describe("private mode and the native clipboard", () => {
  test("blocks capture until the setting is turned off", async () => {
    await openTab("Service");
    const toggle = await app.browser.$("#private-mode");
    await toggle.waitForClickable({ timeout: 15_000 });
    expect(await toggle.getAttribute("aria-checked")).toBe("false");

    await toggle.click();
    await app.browser.waitUntil(
      async () => (await toggle.getAttribute("aria-checked")) === "true",
      { timeout: 10_000, timeoutMsg: "private mode never became active" },
    );

    const blocked = `blocked-windows-clipboard-${Date.now()}`;
    await setClipboard(blocked);
    await app.browser.pause(2_000);
    expect((await app.daemon.items()).some((item) => item.content === blocked)).toBe(
      false,
    );

    await toggle.click();
    await app.browser.waitUntil(
      async () => (await toggle.getAttribute("aria-checked")) === "false",
      { timeout: 10_000, timeoutMsg: "private mode never turned off" },
    );

    const captured = `captured-windows-clipboard-${Date.now()}`;
    await setClipboard(captured);
    await app.browser.waitUntil(
      async () =>
        (await app.daemon.items()).some((item) => item.content === captured),
      { timeout: 15_000, timeoutMsg: "the Windows clipboard was not captured" },
    );
  });
});

describe("desktop settings", () => {
  test("persists a native shortcut registration and restores the default", async () => {
    await openTab("Shortcut");
    const binding = await app.browser.$('[aria-label^="Current shortcut:"]');
    await binding.waitForClickable({ timeout: 15_000 });
    await binding.click();
    await app.browser.keys(["Control", "Shift", "B"]);

    await app.browser.waitUntil(
      async () =>
        (await binding.getAttribute("aria-label"))?.includes(
          "CmdOrCtrl+Shift+B",
        ) === true,
      { timeout: 10_000, timeoutMsg: "the Windows shortcut was not saved" },
    );

    await clickButton(app.browser, "Reset to default");
    await app.browser.waitUntil(
      async () =>
        (await binding.getAttribute("aria-label"))?.includes(
          "CmdOrCtrl+Shift+V",
        ) === true,
      { timeout: 10_000, timeoutMsg: "the default shortcut was not restored" },
    );
  });

  test("changes and restores the Windows launch-at-login registration", async () => {
    await openTab("Shortcut");
    const toggle = await app.browser.$("#open-at-login");
    await toggle.waitForClickable({ timeout: 15_000 });
    const initial = await toggle.getAttribute("aria-checked");
    expect(["true", "false"]).toContain(initial);

    await toggle.click();
    await app.browser.waitUntil(
      async () => (await toggle.getAttribute("aria-checked")) !== initial,
      { timeout: 15_000, timeoutMsg: "launch at login did not change" },
    );

    await toggle.click();
    await app.browser.waitUntil(
      async () => (await toggle.getAttribute("aria-checked")) === initial,
      { timeout: 15_000, timeoutMsg: "launch at login was not restored" },
    );
  });
});

describe("fail-closed product surfaces", () => {
  test("keeps cloud controls absent when no deployment is configured", async () => {
    await openTab("Sync");
    await waitForText(app.browser, "Not configured");
    expect(await app.browser.$$('form[aria-label="Cloud account sign in"]')).toHaveLength(
      0,
    );
  });

  test("warns before readable export and destructive restore", async () => {
    await openTab("Storage");
    await clickButton(app.browser, "Export…");
    await waitForText(app.browser, "Export your clipboard history?");
    const includeSensitive = await app.browser.$("#export-include-sensitive");
    expect(await includeSensitive.getAttribute("aria-checked")).toBe("false");
    expect(await visibleText(app.browser)).toContain(
      "Turning it on writes detected credentials out in the clear.",
    );
    await clickButton(app.browser, "Cancel");

    await clickButton(app.browser, "Restore…");
    await waitForText(app.browser, "Replace this device's history?");
    expect(await visibleText(app.browser)).toContain(
      "checked before anything is replaced",
    );
    await clickButton(app.browser, "Cancel");
  });

  test("collects redacted Windows diagnostics and runtime events", async () => {
    await openTab("Diagnostics");
    await waitForText(app.browser, "windows-system-clipboard", 30_000);
    expectNoFilesystemPath(
      await accessibleSurface(app.browser),
      app.daemon.dataHome,
    );

    const events = await app.browser.$('[role="tab"]=Runtime events');
    await events.click();
    const log = await app.browser.$(
      '[role="log"][aria-label="Runtime event list"]',
    );
    await log.waitForExist({ timeout: 30_000 });
    expect(await log.$$("article")).not.toHaveLength(0);
    expectNoFilesystemPath(
      await accessibleSurface(app.browser),
      app.daemon.dataHome,
    );
  });
});
