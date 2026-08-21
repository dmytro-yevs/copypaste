/**
 * The Devices screen's WebView half on Android.
 *
 * Pairing credentials and the SAS belong to the protected native presenter.
 * This suite therefore proves the WebView offers both ceremonies and reports
 * progress without acquiring a credential surface of its own.
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import {
  accessibleSurface,
  expectNoFilesystemPath,
  expectNoRawError,
  outerHtml,
} from "../src/harness/leaks.js";
import { count, gotoView, visibleText, waitFor } from "../src/harness/ui.js";

/** DMY-48: this leg asserted the string "Nearby devices", which the screen has
 *  never rendered — its heading reads "Discovered on your network". The region
 *  is anchored on the heading that labels it and asserted against the copy the
 *  screen actually shows — see DevicesView.test.tsx, which pins both. */
const DISCOVERED = 'section[aria-labelledby="discovered-devices-heading"]';
const DISCOVERED_HEADING = "Discovered on your network";
const HEADER = "header.chrome";
const PAIRING_CODE = /\b[A-Z0-9]{4}(?:-[A-Z0-9]{4}){3}\b/;
const SECURITY_CODE = /\b[0-9A-F]{6}\b/;
const ACTIONS = ["Show pairing code", "Scan pairing code"];

let app: AndroidApp;

beforeAll(async () => {
  app = await attachToApp();
  await gotoView(app, "Devices");
  await waitFor(
    async () => {
      const text = await visibleText(app);
      return (
        text.includes("This device") &&
        text.includes(DISCOVERED_HEADING) &&
        (await count(app, DISCOVERED)) === 1
      );
    },
    "the Devices screen never settled",
  );
}, 180_000);

afterAll(async () => {
  await app?.detach();
});

async function actionBoxes() {
  return app.withPage((page) =>
    page.evaluate(
      (scope: string, labels: string[]) => {
        const root = document.querySelector(scope);
        return labels.map((label) => {
          const button = Array.from(root?.querySelectorAll("button") ?? []).find(
            (node) => node.getAttribute("aria-label") === label,
          );
          const rect = button?.getBoundingClientRect();
          return {
            label,
            present: Boolean(button),
            width: rect?.width ?? 0,
            height: rect?.height ?? 0,
            right: rect?.right ?? 0,
          };
        });
      },
      HEADER,
      ACTIONS,
    ),
  );
}

describe("the screen", () => {
  test("describes this device and its paired-device region", async () => {
    const text = await visibleText(app);
    expect(text).toContain("This device");
    expect(text).toContain("Paired devices");
    expect(text).toContain(DISCOVERED_HEADING);
    expect(await count(app, DISCOVERED)).toBe(1);
    expectNoRawError(await outerHtml(app));
    expectNoFilesystemPath(await accessibleSurface(app));
  });

  test("offers both native pairing ceremonies", async () => {
    const text = await visibleText(app);
    const surface = await app.withPage((page) =>
      page.evaluate(
        (scope: string, labels: string[]) => {
          const root = document.querySelector(scope);
          return labels.every((label) =>
            Array.from(root?.querySelectorAll("button") ?? []).some(
              (node) => node.getAttribute("aria-label") === label,
            ),
          );
        },
        HEADER,
        ACTIONS,
      ),
    );

    expect(surface).toBe(true);
    expect(text).not.toContain("Ready to pair");
  });

  test("lays both pairing controls out as reachable touch targets", async () => {
    const width = await app.withPage((page) =>
      page.evaluate(() => document.documentElement.clientWidth),
    );
    for (const action of await actionBoxes()) {
      expect(action.present, action.label).toBe(true);
      expect(action.width, action.label).toBeGreaterThan(0);
      expect(action.height, action.label).toBeGreaterThanOrEqual(44);
      expect(action.right, action.label).toBeLessThanOrEqual(width + 1);
    }
  });
});

describe("the native security boundary", () => {
  test("keeps pairing credentials and comparison codes out of the WebView", async () => {
    const surface = await app.withPage((page) =>
      page.evaluate((selector: string) => {
        const root = document.querySelector(selector) as HTMLElement | null;
        return {
          text: root?.innerText ?? "",
          credentialNodes:
            root?.querySelectorAll("input, output, canvas, [data-pairing-secret]").length ?? 0,
        };
      }, HEADER),
    );

    expect(surface.text).not.toMatch(PAIRING_CODE);
    expect(surface.text).not.toMatch(SECURITY_CODE);
    expect(surface.credentialNodes).toBe(0);
  });
});
