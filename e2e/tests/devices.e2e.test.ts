/**
 * Devices and pairing, against a second real daemon.
 *
 * The pairing this drives is the real one: a second `copypaste-daemon` mints a
 * code, the app types it into its own "Add a device" dialog, and the Noise
 * handshake either completes over loopback or the test fails. Nothing here is
 * stubbed, which is the only way to exercise what the screen does with a
 * *wrong* code as well as a right one.
 *
 * Two rules get their real-engine check here:
 *
 *  - **INV-13 — the QR payload must never enter the DOM.** `QrCode` draws to a
 *    canvas precisely so that the credential has no textual representation, and
 *    jsdom cannot tell a canvas that drew from one that silently did not.
 *  - **The camera is allowed to be absent.** Under Xvfb there is no camera at
 *    all, which is the same situation as a desktop without one and a user who
 *    refused the permission: the dialog must fall back to typing rather than
 *    dead-ending.
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import { startDaemon, type Daemon } from "../src/harness/daemon.js";
import {
  accessibleSurface,
  expectNoFilesystemPath,
  expectNoRawError,
  outerHtml,
} from "../src/harness/leaks.js";
import { clickButton, gotoView, visibleText, waitForText } from "../src/harness/ui.js";

interface PairingData {
  code: string;
  pairing_id: string;
  listen_addr: string | null;
}

interface PeerInfo {
  pairing_id: string;
  name: string;
  online: boolean;
  last_seen_ms: number;
}

let app: App;
/** The device on the other end of the pairing. Its own data directory, its own
 *  socket, its own peer port — a second install in every way that matters. */
let other: Daemon;

beforeAll(async () => {
  app = await startApp();
  other = await startDaemon();
  await gotoView(app.browser, "Devices");
  await waitForText(app.browser, "No other devices paired");
}, 300_000);

afterAll(async () => {
  await app?.stop();
  await other?.stop();
});

/** The code's own element, and whether it is readable. */
async function codeField() {
  return (await app.browser.execute(function () {
    const el = document.querySelector("output") as HTMLElement | null;
    if (!el) return null;
    return {
      label: el.getAttribute("aria-label") ?? "",
      text: el.innerText.trim(),
      filter: getComputedStyle(el).filter,
    };
  })) as { label: string; text: string; filter: string } | null;
}

describe("the pairing code", () => {
  test("is minted on request and covered until it is asked for", async () => {
    await clickButton(app.browser, "Pair a new device");
    await waitForText(app.browser, "Generate code");
    await clickButton(app.browser, "Generate code");

    await app.browser.waitUntil(async () => (await codeField()) !== null, {
      timeout: 30_000,
      timeoutMsg: "no pairing code was ever rendered",
    });

    const field = (await codeField())!;
    expect(field.text.length).toBeGreaterThan(20);
    expect(field.label).toContain("hidden");
    // Covered by the engine, not by a class name that might not resolve.
    expect(field.filter).toContain("blur");

    const reveal = await app.browser.$('[aria-label="Reveal the pairing code"]');
    expect(await reveal.isDisplayed()).toBe(true);
  });

  test("warns that it is a credential shown once", async () => {
    const text = await visibleText(app.browser);
    expect(text).toContain("Anyone with this code can pair");
    expect(text).toContain("cannot be retrieved again");
  });

  test("becomes readable only after the reveal (CopyPaste-1jms.2)", async () => {
    await clickButton(app.browser, "Reveal the pairing code");
    await app.browser.waitUntil(
      async () => !((await codeField())?.filter ?? "").includes("blur"),
      { timeout: 10_000, timeoutMsg: "revealing did not uncover the code" },
    );
    expect((await codeField())!.label).toBe("Pairing code");
  });
});

describe("the QR code (INV-13)", () => {
  test("this host is reachable, so the QR path is the one under test", async () => {
    // The QR is only offered when the daemon can name an address; a host with
    // nothing but loopback legitimately renders the text path alone, and the
    // assertions below would then be vacuous rather than failing.
    expect(await visibleText(app.browser)).not.toContain("Not reachable");
  });

  test("is really drawn, not an empty canvas", async () => {
    const drawn = (await app.browser.execute(function () {
      const canvas = document.querySelector(
        'canvas[role="img"]',
      ) as HTMLCanvasElement | null;
      if (!canvas) return null;
      const context = canvas.getContext("2d");
      if (!context) return null;
      const { data } = context.getImageData(0, 0, canvas.width, canvas.height);
      let dark = 0;
      for (let i = 0; i < data.length; i += 4) {
        if (data[i]! < 128) dark += 1;
      }
      return { width: canvas.width, height: canvas.height, dark, total: data.length / 4 };
    })) as { width: number; height: number; dark: number; total: number } | null;

    expect(drawn, "no QR canvas was rendered").not.toBeNull();
    expect(drawn!.width).toBeGreaterThan(100);
    // A blank canvas is all light; a solid one is all dark. A QR is neither.
    const ratio = drawn!.dark / drawn!.total;
    expect(ratio).toBeGreaterThan(0.1);
    expect(ratio).toBeLessThan(0.7);
  });

  test("the payload has no DOM representation at all", async () => {
    const code = (await codeField())!.text;
    const html = await outerHtml(app.browser);

    // The payload is `copypaste://pair?v=2&c=<code>&a=<addr>`. None of it may
    // exist as markup, an attribute or a text node — the pixels are the only
    // copy.
    expect(html).not.toContain("copypaste://pair");
    expect(html).not.toContain(`c=${code}`);
    // The code itself is rendered once, as the code. If it appeared twice, the
    // second one would be the payload.
    expect(html.split(code)).toHaveLength(2);

    const surface = await accessibleSurface(app.browser);
    expect(surface).not.toContain("copypaste://pair");
    expectNoFilesystemPath(surface, app.daemon.dataHome);
  });
});

describe("adding a device", () => {
  test("closing the code dialog and opening the other one", async () => {
    await app.browser.keys(["Escape"]);
    await app.browser.waitUntil(async () => (await codeField()) === null, {
      timeout: 10_000,
      timeoutMsg: "the pairing dialog would not close",
    });

    await clickButton(app.browser, "Add a device");
    await waitForText(app.browser, "Scan the code shown on the other device");
  });

  test("degrades to manual entry when there is no camera", async () => {
    await clickButton(app.browser, "Scan a code");
    await waitForText(app.browser, "No camera is available", 30_000);

    // The fallback has to be usable, not merely announced.
    for (const id of ["#accept-code", "#accept-addr"]) {
      const input = await app.browser.$(id);
      expect(await input.isDisplayed(), id).toBe(true);
      expect(await input.isEnabled(), id).toBe(true);
    }
  });

  test("a wrong code fails visibly and changes nothing", async () => {
    const wrong = "WRNG-WRNG-WRNG-WRNG-WRNG-WRNG-WRNG-WRNG-WRNG-WRNG-WRNG-WRNG-WRNG";
    await app.browser.$("#accept-code").setValue(wrong);
    await app.browser.$("#accept-addr").setValue(`127.0.0.1:${other.peerPort}`);
    await clickButton(app.browser, "Pair");

    await waitForText(app.browser, "Nothing was changed", 45_000);

    const alert = await app.browser.$('[role="alert"]');
    expect(await alert.isDisplayed()).toBe(true);
    expectNoRawError(await outerHtml(app.browser));
    expectNoFilesystemPath(
      await accessibleSurface(app.browser),
      app.daemon.dataHome,
      other.dataHome,
    );

    const peers = await other.json<PeerInfo[]>(["peers"]);
    expect(
      peers.some((peer) => peer.name.includes("e2e")),
      "a refused pairing left something behind on the other device",
    ).toBe(false);
  });

  test("the right code pairs the two devices for real", async () => {
    const minted = await other.json<PairingData>([
      "pair",
      "create",
      "-n",
      "the app under test",
    ]);

    await app.browser.$("#accept-code").setValue(minted.code);
    await app.browser.$("#accept-addr").setValue(`127.0.0.1:${other.peerPort}`);
    await clickButton(app.browser, "Pair");

    // The dialog closes only on success, so its disappearance is the outcome.
    await app.browser.waitUntil(
      async () =>
        !(await visibleText(app.browser)).includes(
          "Scan the code shown on the other device",
        ),
      { timeout: 60_000, interval: 500, timeoutMsg: "the pairing never completed" },
    );

    // Both ends agree, which a UI-only assertion could not establish.
    const ours = await app.daemon.json<PeerInfo[]>(["peers"]);
    expect(ours.some((peer) => peer.pairing_id === minted.pairing_id)).toBe(true);
    const theirs = await other.json<PeerInfo[]>(["peers"]);
    expect(theirs.length).toBeGreaterThan(0);
  });

  test("the paired device is listed, and its name is labelled unverified (INV-15)", async () => {
    const paired = (await app.daemon.json<PeerInfo[]>(["peers"])).find(
      (peer) => peer.online,
    )!;
    await waitForText(app.browser, paired.name, 30_000);
    // `last_seen_ms` is a last-*synced* time, and a pairing that has not synced
    // renders "Never synced" rather than an age off the epoch.
    await waitForText(app.browser, paired.last_seen_ms > 0 ? "Last synced" : "Never synced");

    const hint = await app.browser.$(
      '[title="Name reported by the device itself — not verified"]',
    );
    expect(await hint.isDisplayed()).toBe(true);
  });
});

describe("unpairing", () => {
  test("asks first, and says that it is one-sided", async () => {
    const paired = (await app.daemon.json<PeerInfo[]>(["peers"])).find(
      (peer) => peer.online,
    )!;

    await clickButton(app.browser, `Unpair ${paired.name}`);
    const dialog = await app.browser.$('[role="alertdialog"]');
    await dialog.waitForDisplayed({
      timeout: 10_000,
      timeoutMsg: "unpairing did not ask for confirmation",
    });
    const copy = await dialog.getText();
    expect(copy).toContain("Unpair");
    expect(copy).toContain("one-sided");
    expect(copy).toContain("Nothing already synced is deleted");
  });

  test("removes the device from the screen and from the store", async () => {
    const before = await app.daemon.json<PeerInfo[]>(["peers"]);
    await clickButton(app.browser, "Unpair", { within: '[role="alertdialog"]' });

    await app.browser.waitUntil(
      async () =>
        (await app.daemon.json<PeerInfo[]>(["peers"])).length < before.length,
      { timeout: 20_000, timeoutMsg: "the peer store still holds the device" },
    );

    const gone = (await app.daemon.json<PeerInfo[]>(["peers"])).every(
      (peer) => !peer.online,
    );
    expect(gone).toBe(true);
  });
});
