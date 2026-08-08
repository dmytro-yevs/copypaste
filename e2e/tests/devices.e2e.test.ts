/**
 * The Devices security boundary and established-device management.
 *
 * A second real daemon and the CLI establish the peer fixture; that setup is
 * not browser coverage for pairing. The browser proves that both pairing flows
 * are offered without a credential reaching the page, and that a known device
 * can sync, expose its revoke confirmation, and unpair.
 *
 * ADR-0007 is Accepted and forbids exactly the two controls the first describe
 * block now requires. The assertions follow the shipped app; the ADR has not
 * been superseded, and the difference is not this file's to settle.
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
}

interface PeerInfo {
  pairing_id: string;
  name: string;
  online: boolean;
  last_seen_ms: number;
}

let app: App;
let other: Daemon;
let paired: PeerInfo;

beforeAll(async () => {
  app = await startApp();
  other = await startDaemon();

  const minted = await other.json<PairingData>([
    "pair",
    "create",
    "-n",
    "the browser app",
  ]);
  await app.daemon.json<unknown>([
    "pair",
    "accept",
    minted.code,
    "--addr",
    `127.0.0.1:${other.peerPort}`,
  ]);

  const peers = await app.daemon.json<PeerInfo[]>(["peers"]);
  const known = peers.find((peer) => peer.pairing_id === minted.pairing_id);
  if (!known) {
    throw new Error("the CLI pairing fixture did not persist on the app daemon");
  }
  paired = known;

  await gotoView(app.browser, "Devices");
  await waitForText(app.browser, paired.name);
}, 300_000);

afterAll(async () => {
  await app?.stop();
  await other?.stop();
});

async function interactiveSurface(): Promise<string> {
  return (await app.browser.execute(function () {
    return Array.from(
      document.querySelectorAll(
        'button, input, select, textarea, [role="button"], [role="link"]',
      ),
      (node) => {
        const element = node as HTMLElement;
        return [
          element.innerText,
          element.getAttribute("aria-label"),
          element.getAttribute("title"),
          element.getAttribute("placeholder"),
        ]
          .filter(Boolean)
          .join(" ");
      },
    ).join("\n");
  })) as string;
}

describe("pairing availability (ADR-0007)", () => {
  test("offers both pairing flows", async () => {
    const text = await visibleText(app.browser);
    expect(text).toContain("Join device");
    expect(text).toContain("Show code");
  });

  test("labels pairing controls without exposing a credential", async () => {
    const controls = await interactiveSurface();
    expect(controls).toMatch(/join device/i);
    expect(controls).toMatch(/show code/i);

    const pairingArtifacts = (await app.browser.execute(function () {
      return document.querySelectorAll(
        'output, #accept-code, #accept-addr, canvas[role="img"]',
      ).length;
    })) as number;
    expect(pairingArtifacts).toBe(0);
  });
});

describe("a known device", () => {
  test("is listed with an explicitly unverified name", async () => {
    await waitForText(
      app.browser,
      paired.last_seen_ms > 0 ? "Last synced" : "Never synced",
    );

    const hint = await app.browser.$(
      '[title="Name reported by the device itself — not verified"]',
    );
    expect(await hint.isDisplayed()).toBe(true);

    for (const label of [
      `Sync with ${paired.name} now`,
      `Unpair ${paired.name}`,
      `Revoke ${paired.name}`,
    ]) {
      const action = await app.browser.$(`button[aria-label="${label}"]`);
      expect(await action.isDisplayed(), label).toBe(true);
      expect(await action.isEnabled(), label).toBe(true);
    }
  });

  test("can run a real sync from the browser", async () => {
    await clickButton(app.browser, `Sync with ${paired.name} now`);
    await waitForText(app.browser, "Last sync from here", 45_000);

    expectNoRawError(await outerHtml(app.browser));
    expectNoFilesystemPath(
      await accessibleSurface(app.browser),
      app.daemon.dataHome,
      other.dataHome,
    );
  });

  test("offers the irreversible revoke confirmation", async () => {
    await clickButton(app.browser, `Revoke ${paired.name}`);

    const dialog = await app.browser.$('[role="alertdialog"]');
    await dialog.waitForDisplayed({
      timeout: 10_000,
      timeoutMsg: "revoking did not ask for confirmation",
    });
    const copy = await dialog.getText();
    expect(copy).toContain("pairing code stops working for good");
    expect(copy).toContain("Nothing already synced is deleted");
    expect(copy).toContain("can't be undone");

    const confirm = await dialog.$("button=Revoke");
    expect(await confirm.isEnabled()).toBe(false);
    await clickButton(app.browser, "Cancel", { within: '[role="alertdialog"]' });
    await dialog.waitForDisplayed({ reverse: true, timeout: 10_000 });
  });
});

describe("unpairing", () => {
  test("asks first, and says that it is one-sided", async () => {
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

  test("removes the device from the screen and the store", async () => {
    await clickButton(app.browser, "Unpair", { within: '[role="alertdialog"]' });

    await app.browser.waitUntil(
      async () => {
        const peers = await app.daemon.json<PeerInfo[]>(["peers"]);
        return peers.every((peer) => peer.pairing_id !== paired.pairing_id);
      },
      { timeout: 20_000, timeoutMsg: "the peer store still holds the device" },
    );
    await waitForText(app.browser, "No other devices paired");
  });
});
