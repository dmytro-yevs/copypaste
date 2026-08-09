/**
 * The Devices security boundary and established-device management.
 *
 * A second real daemon and the CLI establish the peer fixture; that setup is
 * not browser coverage for the native renderer. The browser proves that it can
 * start the native-safe commands without any credential reaching the page, and
 * that a known device can sync, expose its revoke confirmation, and unpair.
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import { setTimeout as sleep } from "node:timers/promises";

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
  expires_in_secs: number;
}

interface PairingProgress {
  pairing_id: string | null;
  state: string;
  sas: string | null;
  known_device: PeerInfo | null;
}

interface PeerInfo {
  pairing_id: string;
  name: string;
  last_addr: string | null;
  online: boolean;
  last_seen_ms: number;
}

interface SyncResult {
  pairing_id: string;
  sent: number;
  received: number;
  error: string | null;
}

let app: App;
let other: Daemon;
let paired: PeerInfo;

async function waitForPairing(
  daemon: Daemon,
  state: string,
): Promise<PairingProgress> {
  const deadline = Date.now() + 20_000;
  for (;;) {
    const progress = await daemon.json<PairingProgress>(["pair", "progress"]);
    if (progress.state === state) return progress;
    if (Date.now() >= deadline) {
      throw new Error(`pairing stayed ${progress.state}; expected ${state}`);
    }
    await sleep(100);
  }
}

async function waitForPeerPresence(
  daemon: Daemon,
  pairingId: string,
  online: boolean,
): Promise<PeerInfo> {
  const deadline = Date.now() + 45_000;
  for (;;) {
    const peer = (await daemon.json<PeerInfo[]>(["peers"])).find(
      (candidate) => candidate.pairing_id === pairingId,
    );
    if (peer?.online === online) return peer;
    if (Date.now() >= deadline) {
      throw new Error(
        `peer ${pairingId} stayed ${peer?.online === true ? "online" : "offline"}; ` +
          `expected ${online ? "online" : "offline"}`,
      );
    }
    await sleep(100);
  }
}

async function completePairing(
  inviter: Daemon,
  joiner: Daemon,
  invite: PairingData,
): Promise<PeerInfo> {
  const joined = await joiner.json<PairingProgress>([
    "pair",
    "join",
    invite.code,
    "--addr",
    `127.0.0.1:${inviter.peerPort}`,
  ]);
  const inbound = await waitForPairing(inviter, "awaiting_confirmation");
  expect(joined.state).toBe("awaiting_confirmation");
  expect(joined.pairing_id).toBe(invite.pairing_id);
  expect(joined.sas).toMatch(/^\d{6}$/);
  expect(inbound.sas).toBe(joined.sas);

  await Promise.all([
    inviter.json<PairingProgress>(["pair", "confirm"]),
    joiner.json<PairingProgress>(["pair", "confirm"]),
  ]);
  const [inviterDone, joinerDone] = await Promise.all([
    waitForPairing(inviter, "confirmed"),
    waitForPairing(joiner, "confirmed"),
  ]);
  expect(inviterDone.known_device).not.toBeNull();
  if (!joinerDone.known_device) throw new Error("pairing did not persist on the joiner");
  return joinerDone.known_device;
}

async function syncUntilPresent(
  fromApp: string,
  fromOther: string,
): Promise<void> {
  const observations: string[] = [];
  const attempts: Array<[string, Daemon]> = [
    ["app", app.daemon],
    ["other", other],
    ["app", app.daemon],
    ["other", other],
  ];
  for (const [label, daemon] of attempts) {
    const results = await daemon.json<SyncResult[]>([
      "sync",
      "--peer",
      paired.pairing_id,
    ]);
    const result = results.find(
      (candidate) => candidate.pairing_id === paired.pairing_id,
    );
    const [here, there] = await Promise.all([
      app.daemon.items(),
      other.items(),
    ]);
    const appReceived = here.some((item) => item.content === fromOther);
    const otherReceived = there.some((item) => item.content === fromApp);
    observations.push(
      result
        ? `${label}: sent=${result.sent}, received=${result.received}, ` +
            `error=${result.error ?? "none"}, app=${appReceived}, ` +
            `other=${otherReceived}`
        : `${label}: requested peer omitted, app=${appReceived}, ` +
            `other=${otherReceived}`,
    );
    if (appReceived && otherReceived) {
      return;
    }
  }

  throw new Error(
    `two-way native sync did not converge: ${observations.join("; ")}`,
  );
}

beforeAll(async () => {
  app = await startApp();
  other = await startDaemon();

  const minted = await other.json<PairingData>(["pair", "create"]);
  const wrong = await app.daemon.cli([
    "pair",
    "join",
    `${minted.code.slice(0, -1)}${minted.code.endsWith("X") ? "Y" : "X"}`,
    "--addr",
    `127.0.0.1:${other.peerPort}`,
  ]);
  expect(wrong.exitCode).not.toBe(0);
  expect(await app.daemon.json<PeerInfo[]>(["peers"])).toEqual([]);

  const confirmed = await completePairing(other, app.daemon, minted);
  paired = await waitForPeerPresence(app.daemon, minted.pairing_id, true);
  expect(paired).toMatchObject({
    pairing_id: confirmed.pairing_id,
    name: confirmed.name,
    last_addr: confirmed.last_addr,
  });
  expect(paired.last_seen_ms).toBeGreaterThan(0);

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
    const minted = await other.json<PairingData>(["pair", "create"]);
    try {
      await clickButton(app.browser, "Refresh devices discovered on this network");

      const html = await outerHtml(app.browser);
      expect(html).not.toContain(minted.code);
      expect(html).not.toContain(minted.code.replace(/-/g, ""));
      expect(html).not.toContain("copypaste://pair");
    } finally {
      await other.json<PairingProgress>(["pair", "cancel"]);
    }
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
  test("reports online and offline network presence", async () => {
    await waitForText(app.browser, "On this network");

    await app.daemon.json<unknown>([
      "config",
      "set",
      "--lan-visibility",
      "false",
    ]);
    try {
      await waitForPeerPresence(app.daemon, paired.pairing_id, false);
      await waitForText(app.browser, "Not seen on this network", 30_000);
    } finally {
      await app.daemon.json<unknown>([
        "config",
        "set",
        "--lan-visibility",
        "true",
      ]);
    }

    paired = await waitForPeerPresence(app.daemon, paired.pairing_id, true);
    await waitForText(app.browser, "On this network", 30_000);
  }, 90_000);

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
    // Numeric uniqueness can form a Luhn-valid card and make the fixture
    // correctly ineligible for sync (Browser CI 31331955801).
    const fromApp = "two-way sync item from the app";
    const fromOther = "two-way sync item from the other device";
    await app.daemon.add(fromApp);
    await other.add(fromOther);

    await syncUntilPresent(fromApp, fromOther);
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
      const minted = await expiring.json<PairingData>(["pair", "create"]);
      await sleep((minted.expires_in_secs + 1) * 1_000);
      const result = await app.daemon.cli([
        "pair",
        "join",
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
  }, 180_000);
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
    const minted = await other.json<PairingData>(["pair", "create"]);
    await completePairing(other, app.daemon, minted);
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
