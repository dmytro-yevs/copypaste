/**
 * The Devices security boundary and established-device management.
 *
 * A second real daemon and the CLI establish the peer fixture; that setup is
 * not browser coverage for the native renderer. The browser proves that it can
 * start the native-safe commands without any credential reaching the page, and
 * that a known device can sync, expose its revoke confirmation, and unpair.
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
  const wrong = await app.daemon.cli([
    "pair",
    "accept",
    `${minted.code.slice(0, -1)}${minted.code.endsWith("X") ? "Y" : "X"}`,
    "--addr",
    `127.0.0.1:${other.peerPort}`,
  ]);
  expect(wrong.exitCode).not.toBe(0);
  expect(await app.daemon.json<PeerInfo[]>(["peers"])).toEqual([]);

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

describe("native-safe pairing", () => {
  test("offers both native entry points", async () => {
    const text = await visibleText(app.browser);
    expect(text).toMatch(/add a device/i);
    expect(text).toMatch(/protected native view/i);
    const controls = await interactiveSurface();
    expect(controls).toMatch(/show pairing code/i);
    expect(controls).toMatch(/scan pairing code/i);
    expect(controls).not.toMatch(/paste pairing details|copy pairing details/i);
  });

  test("renders no QR, credential field or pairing dialog", async () => {
    const artifacts = (await app.browser.execute(function () {
      return document.querySelectorAll(
        'output, #accept-code, #accept-addr, #pairing-code, #pairing-address,' +
          ' #pairing-security-code, canvas, svg[role="img"], [role="dialog"]',
      ).length;
    })) as number;
    expect(artifacts).toBe(0);
  });

  /** The credential must be absent from the document, not merely unrendered: a
   *  blur, a `display: none` or an `aria-label` all leave it in `outerHTML`. */
  test("never lets a pairing credential reach the page", async () => {
    const minted = await other.json<PairingData>([
      "pair",
      "create",
      "-n",
      "leak probe",
    ]);
    await clickButton(app.browser, "Refresh devices discovered on this network");

    const html = await outerHtml(app.browser);
    expect(html).not.toContain(minted.code);
    expect(html).not.toContain(minted.code.replace(/-/g, ""));
    expect(html).not.toContain("copypaste://pair");
  });

  test("a browser without native presentation cancels safely", async () => {
    await clickButton(app.browser, "Show pairing code");
    await waitForText(app.browser, "Pairing cancelled");

    const surface = await accessibleSurface(app.browser);
    expect(surface).toMatch(/protected pairing view didn't open/i);
    expectNoFilesystemPath(surface);
    expectNoRawError(surface);
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

  test("moves clipboard history both ways", async () => {
    const fromApp = `from-app-${Date.now()}`;
    const fromOther = `from-other-${Date.now()}`;
    await app.daemon.add(fromApp);
    await other.add(fromOther);

    await app.daemon.json<unknown>(["sync", "--peer", paired.pairing_id]);
    await other.json<unknown>(["sync", "--peer", paired.pairing_id]);
    await app.browser.waitUntil(
      async () => {
        const [here, there] = await Promise.all([
          app.daemon.items(),
          other.items(),
        ]);
        return (
          here.some((item) => item.content === fromOther) &&
          there.some((item) => item.content === fromApp)
        );
      },
      { timeout: 45_000, timeoutMsg: "two-way native sync did not converge" },
    );
  });

  test("shows a failure and recovers after reconnect", async () => {
    await other.kill();
    await clickButton(app.browser, `Sync with ${paired.name} now`);
    await waitForText(app.browser, "Sync failed", 45_000);

    await other.restart();
    await clickButton(app.browser, `Sync with ${paired.name} now`);
    await waitForText(app.browser, "Last sync from here", 45_000);
    expectNoRawError(await outerHtml(app.browser));
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

describe("an expired code", () => {
  test("is refused without leaving a phantom peer", async () => {
    const expiring = await startDaemon();
    try {
      const minted = await expiring.json<PairingData>([
        "pair",
        "create",
        "-n",
        "expired fixture",
      ]);
      await expiring.expirePairing(minted.pairing_id);
      const result = await app.daemon.cli([
        "pair",
        "accept",
        minted.code,
        "--addr",
        `127.0.0.1:${expiring.peerPort}`,
      ]);
      expect(result.exitCode).not.toBe(0);
      expect(
        (await app.daemon.json<PeerInfo[]>(["peers"])).some(
          (peer) => peer.pairing_id === minted.pairing_id,
        ),
      ).toBe(false);
    } finally {
      await expiring.stop();
    }
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

describe("revoking", () => {
  test("blocks a newly established pairing and removes it from the UI", async () => {
    const minted = await other.json<PairingData>([
      "pair",
      "create",
      "-n",
      "revoke fixture",
    ]);
    await app.daemon.json<unknown>([
      "pair",
      "accept",
      minted.code,
      "--addr",
      `127.0.0.1:${other.peerPort}`,
    ]);
    const peer = (await app.daemon.json<PeerInfo[]>(["peers"])).find(
      (candidate) => candidate.pairing_id === minted.pairing_id,
    );
    if (!peer) throw new Error("the revoke fixture did not establish");
    await clickButton(app.browser, "Refresh devices discovered on this network");

    await clickButton(app.browser, `Revoke ${peer.name}`);
    const acknowledgement = await app.browser.$('label[for="revoke-ack"]');
    await acknowledgement.waitForClickable({ timeout: 10_000 });
    await acknowledgement.click();
    await clickButton(app.browser, "Revoke", { within: '[role="alertdialog"]' });

    await app.browser.waitUntil(
      async () =>
        (await app.daemon.json<PeerInfo[]>(["peers"])).every(
          (candidate) => candidate.pairing_id !== minted.pairing_id,
        ),
      { timeout: 20_000, timeoutMsg: "the revoked peer remained in the store" },
    );
    await waitForText(app.browser, "No other devices paired");
  });
});
