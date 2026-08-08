/**
 * The Devices screen and the pairing ceremony, on the shipping Android engine.
 *
 * CLAUDE.md rule 6 is the argument for putting this ahead of export and push:
 * pairing is the feature that once shipped with no interface at all, and the
 * phone is the device a person actually pairs from. The mint side runs
 * end to end here — Android links the core in-process (ADR-0003), so "Show
 * code" reaches the real peer listener with no daemon and no CLI.
 *
 * What this cannot reach is the other half of the ceremony. Accepting a
 * pairing needs a second device on the network; the browser layer supplies one
 * as a CLI fixture and there is no counterpart on a phone, so `devices` here
 * asserts the join form is offered and correct, never that a join succeeds.
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import {
  accessibleSurface,
  expectNoFilesystemPath,
  expectNoRawError,
  outerHtml,
} from "../src/harness/leaks.js";
import { count, gotoView, tapButton, visibleText, waitFor } from "../src/harness/ui.js";

const DIALOG = '[role="dialog"]';

/** The pairing code's own shape — groups of four, hyphenated. Used to assert
 *  the secret is not on screen before it is asked for, and is gone after. */
const PAIRING_CODE = /\b[A-Z0-9]{4}-[A-Z0-9]{4}-[A-Z0-9]{4}-[A-Z0-9]{4}\b/;

let app: AndroidApp;

beforeAll(async () => {
  app = await attachToApp();
  await gotoView(app, "Devices");
  await waitFor(
    async () => (await visibleText(app)).includes("This device"),
    "the Devices screen never rendered",
  );
}, 180_000);

afterAll(async () => {
  await closeDialog().catch(() => undefined);
  await app?.detach();
});

async function closeDialog(): Promise<void> {
  if ((await count(app, DIALOG)) === 0) return;
  await tapButton(app, "Close", { within: DIALOG });
  await waitFor(async () => (await count(app, DIALOG)) === 0, "the dialog never closed");
}

async function dialogText(
  ready: (text: string) => boolean = (text) => text.length > 0,
  describe = "the dialog never rendered any text",
): Promise<string> {
  let text = "";
  await waitFor(async () => {
    text = await app.withPage((page) =>
      page.evaluate(
        (selector: string) =>
          (document.querySelector(selector) as HTMLElement | null)?.innerText ?? "",
        DIALOG,
      ),
    );
    return ready(text);
  }, describe);
  return text;
}

describe("the screen itself", () => {
  test("describes this device and the absence of peers, in rendered words", async () => {
    const text = await visibleText(app);
    expect(text).toContain("This device");
    expect(text).toContain("No other devices paired");

    expectNoRawError(await outerHtml(app));
    expectNoFilesystemPath(await accessibleSurface(app));
  });

  test("offers both pairing flows (ADR-0007)", async () => {
    const text = await visibleText(app);
    expect(text).toContain("Join device");
    expect(text).toContain("Show code");
  });

  test("shows no pairing credential before one is asked for", async () => {
    expect(await visibleText(app)).not.toMatch(PAIRING_CODE);
    // The browser layer's artefact check, in Android's vocabulary: nothing is
    // pre-rendered waiting to be filled in.
    expect(await count(app, "output, #pairing-code, #pairing-address")).toBe(0);
  });
});

describe("minting a code", () => {
  test("shows a code, a QR and a security code, and says it is shown once", async () => {
    await tapButton(app, "Show code");
    // The dialog opens on "Creating a pairing code…" and the mint is a real
    // round trip into the in-process core, which is the part worth waiting for.
    const text = await dialogText(
      (shown) => PAIRING_CODE.test(shown),
      "the pairing code was never minted",
    );

    expect(text).toMatch(PAIRING_CODE);
    expect(text).toContain("shown once");
    // The SAS the ceremony is bound to. Six characters, and the screen has to
    // tell the user to compare it rather than merely printing it.
    expect(text).toMatch(/\b[0-9A-F]{6}\b/);
    expect(text).toContain("Compare the six-character security code");

    // INV-13: the QR is drawn, with a box, on this engine. jsdom renders every
    // rect at 0×0 and would agree with a QR that never painted.
    const qr = await app.withPage((page) =>
      page.evaluate((selector: string) => {
        const dialog = document.querySelector(selector);
        const svg = dialog?.querySelector("svg[viewBox]") as SVGElement | null;
        if (!svg) return null;
        const rect = svg.getBoundingClientRect();
        return { width: rect.width, height: rect.height, cells: svg.querySelectorAll("*").length };
      }, DIALOG),
    );
    expect(qr).not.toBeNull();
    expect(qr!.width).toBeGreaterThan(80);
    expect(qr!.height).toBeGreaterThan(80);
    expect(qr!.cells).toBeGreaterThan(1);
  }, 60_000);

  test("closing it takes the code out of the document, not merely off the screen", async () => {
    await closeDialog();
    // `outerHTML`, not the rendered text: a code left in a hidden node is a
    // code a screen reader still reaches (INV-10's rule, applied to the one
    // secret this screen mints).
    expect(await outerHtml(app)).not.toMatch(PAIRING_CODE);
  });
});

describe("joining another device", () => {
  test("asks for the code, the address and the security code, and gates on comparing it", async () => {
    await tapButton(app, "Join device");
    const text = await dialogText();

    expect(text).toContain("Pairing code");
    expect(text).toContain("Connection address");
    expect(text).toContain("Security code");
    expect(text).toContain("I compared the security code on both devices.");

    const fields = await app.withPage((page) =>
      page.evaluate(
        (selector: string) =>
          Array.from(
            document.querySelectorAll(`${selector} input`),
            (node) => (node as HTMLInputElement).id,
          ),
        DIALOG,
      ),
    );
    expect(fields).toEqual(["pairing-code", "pairing-address", "pairing-security-code"]);

    expectNoFilesystemPath(await accessibleSurface(app));
    await closeDialog();
  }, 60_000);
});
