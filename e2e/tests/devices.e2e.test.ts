/**
 * The Connections security boundary and established-device management.
 *
 * A second real daemon and the CLI establish the peer fixture; that setup is
 * not browser coverage for the native renderer. The browser proves that it can
 * start the native-safe commands without any credential reaching the page, and
 * that a known device can sync, expose its revoke confirmation, and unpair.
 */
import { existsSync } from "node:fs";
import path from "node:path";

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
  listen_addr: string | null;
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
  expect(existsSync(path.join(app.daemon.dataHome, "peers.json"))).toBe(true);
  expect(existsSync(path.join(other.dataHome, "peers.json"))).toBe(true);

  await gotoView(app.browser, "Connections");
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

async function openPairingChoices(): Promise<void> {
  await clickButton(app.browser, "Connect a device");
  await waitForText(
    app.browser,
    process.platform === "linux"
      ? "Protected pairing requires a current native CopyPaste build."
      : "Choose how to open the protected pairing flow on this device.",
  );
}

async function closePairingChoices(): Promise<void> {
  const close = await app.browser.$('[data-slot="dialog-close"]');
  await close.waitForClickable({ timeout: 10_000 });
  await close.click();
  await app.browser.waitUntil(
    async () =>
      !(await visibleText(app.browser)).includes(
        process.platform === "linux"
          ? "Protected pairing requires a current native CopyPaste build."
          : "Choose how to open the protected pairing flow on this device.",
      ),
    { timeout: 10_000, timeoutMsg: "pairing choices did not close" },
  );
}

function peerCardSelector(pairingId: string): string {
  return `button[data-device-selection-key="peer:${pairingId}"]`;
}

interface RosterState {
  peerPresent: boolean;
  capacity: string | null;
  capacityLabel: string | null;
}

async function readRosterState(pairingId: string): Promise<RosterState> {
  return (await app.browser.execute(function (expectedPairingId) {
    const peerPresent = Array.from(
      document.querySelectorAll("button[data-device-selection-key]"),
    ).some(
      (card) =>
        card.getAttribute("data-device-selection-key") ===
        `peer:${expectedPairingId}`,
    );
    const note = document.querySelector(
      'section[aria-labelledby="your-devices-heading"] > p',
    );
    return {
      peerPresent,
      capacity: note?.querySelector("strong")?.textContent?.trim() ?? null,
      capacityLabel:
        note?.querySelector("strong + span")?.textContent?.trim() ?? null,
    };
  }, pairingId)) as RosterState;
}

async function waitForRosterRemoval(
  pairingId: string,
  expectedCapacity: number,
): Promise<void> {
  const expectedLabel =
    `more device pairing${expectedCapacity === 1 ? "" : "s"} available.`;
  const deadline = Date.now() + 20_000;
  for (;;) {
    const observed = await readRosterState(pairingId);
    if (
      !observed.peerPresent &&
      observed.capacity === String(expectedCapacity) &&
      observed.capacityLabel === expectedLabel
    ) {
      return;
    }
    if (Date.now() >= deadline) {
      throw new Error(
        `device roster did not remove ${pairingId} and return capacity to ` +
          `${expectedCapacity}; last state: ${JSON.stringify(observed)}`,
      );
    }
    await sleep(100);
  }
}

async function openPeerDetails(peer: Pick<PeerInfo, "pairing_id" | "name">) {
  const details = await app.browser.$('section[aria-label$=" details"]');
  if (await details.isDisplayed()) {
    expect(await details.getText()).toContain(peer.name);
    return details;
  }

  const card = await app.browser.$(peerCardSelector(peer.pairing_id));
  await card.waitForClickable({
    timeout: 30_000,
    timeoutMsg: `the paired device card for ${peer.name} did not render`,
  });
  await card.click();

  await details.waitForDisplayed({
    timeout: 10_000,
    timeoutMsg: `details for ${peer.name} did not open`,
  });
  return details;
}

async function waitForManualSync(): Promise<void> {
  await app.browser.waitUntil(
    async () => {
      const text = await visibleText(app.browser);
      return (
        text.includes("Last sync requested here") &&
        !text.includes("No sync requested here yet")
      );
    },
    { timeout: 45_000, timeoutMsg: "the browser did not record the sync" },
  );
}

function pairingUri(invite: PairingData): string {
  const uri = new URL("copypaste://pair");
  uri.searchParams.set("code", invite.code);
  uri.searchParams.set("id", invite.pairing_id);
  if (invite.listen_addr !== null) {
    uri.searchParams.set("addr", invite.listen_addr);
  }
  return uri.toString();
}

describe("native-safe pairing", () => {
  test("reports launcher availability and entry-point wording", async () => {
    await openPairingChoices();
    const text = await visibleText(app.browser);
    expect(text).toMatch(/connect a device/i);
    const controls = await interactiveSurface();
    if (process.platform === "linux") {
      expect(text).toMatch(
        /protected pairing requires a current native copypaste build/i,
      );
      expect(controls).not.toMatch(/show pairing code|enter pairing code/i);
    } else {
      expect(text).toMatch(/protected pairing flow/i);
      expect(controls).toMatch(/show pairing code/i);
      expect(controls).toMatch(/enter pairing code/i);
    }
    expect(controls).not.toMatch(/paste pairing details|copy pairing details/i);
    await closePairingChoices();
  });

  test("renders no QR or pairing credentials in the launcher", async () => {
    await openPairingChoices();
    const artifacts = (await app.browser.execute(function () {
      return document.querySelectorAll(
        'output, #pairing-code, #pairing-address,' +
          ' #pairing-security-code, canvas, img[alt="Pairing QR code"],' +
          ' img[alt="QR code"], svg[role="img"][aria-label="Pairing QR code"],' +
          ' [data-pairing-secret]',
      ).length;
    })) as number;
    expect(artifacts).toBe(0);
    await closePairingChoices();
  });

  /** The credential must be absent from the document, not merely unrendered: a
   *  blur, a `display: none` or an `aria-label` all leave it in `outerHTML`. */
  test("never lets a pairing credential reach the page", async () => {
    const minted = await app.daemon.json<PairingData>(["pair", "create"]);
    const uri = pairingUri(minted);
    try {
      await clickButton(app.browser, "Refresh");

      const html = await outerHtml(app.browser);
      const surface = await accessibleSurface(app.browser);
      for (const secret of [minted.code, minted.code.replace(/-/g, ""), uri]) {
        expect(html).not.toContain(secret);
        expect(surface).not.toContain(secret);
      }
    } finally {
      await app.daemon.json<PairingProgress>(["pair", "cancel"]);
    }
  });

  test("executes native pairing and handles its platform boundary", async () => {
    if (process.platform !== "linux" && process.platform !== "win32") {
      throw new Error(
        `native pairing E2E is only wired for Linux WebKitGTK and Windows WebView2 (got ${process.platform})`,
      );
    }
    await openPairingChoices();

    if (process.platform === "linux") {
      const surface = await accessibleSurface(app.browser);
      expect(surface).toMatch(
        /protected pairing requires a current native copypaste build/i,
      );
      expect(surface).not.toMatch(/show pairing code|enter pairing code/i);
      expectNoFilesystemPath(surface);
      expectNoRawError(surface);
      const html = await outerHtml(app.browser);
      expect(html).not.toMatch(/Pairing QR code|pairing-security-code/i);
      await closePairingChoices();
      return;
    }

    await clickButton(app.browser, "Show pairing code");

    if (process.platform === "win32") {
      const waiting = await waitForPairing(app.daemon, "waiting_for_peer");
      expect(waiting.state).toBe("waiting_for_peer");

      // This is backend cleanup, not evidence that the native Close action
      // invoked the abort callback. That behavior belongs to the Windows
      // native pairing gate.
      const cancelled = await app.daemon.json<PairingProgress>([
        "pair",
        "cancel",
      ]);
      expect(cancelled.state).toBe("cancelled");
      await waitForPairing(app.daemon, "cancelled");
    } else {
      const cancelled = await waitForPairing(app.daemon, "cancelled");
      expect(cancelled.state).toBe("cancelled");
    }

    const surface = await accessibleSurface(app.browser);
    expectNoFilesystemPath(surface);
    expectNoRawError(surface);
    const html = await outerHtml(app.browser);
    expect(html).not.toMatch(/Pairing QR code|pairing-security-code/i);
  });
});

describe("a known device", () => {
  test("reports online and offline network presence", async () => {
    await openPeerDetails(paired);
    await waitForText(app.browser, "Seen on this network");

    await app.daemon.json<unknown>([
      "config",
      "set",
      "--lan-visibility",
      "false",
    ]);
    try {
      await waitForPeerPresence(app.daemon, paired.pairing_id, false);
      await waitForText(app.browser, "Not currently discovered", 30_000);
    } finally {
      await app.daemon.json<unknown>([
        "config",
        "set",
        "--lan-visibility",
        "true",
      ]);
    }

    paired = await waitForPeerPresence(app.daemon, paired.pairing_id, true);
    await waitForText(app.browser, "Seen on this network", 30_000);
  }, 90_000);

  test("is listed with an explicitly unverified name", async () => {
    const details = await openPeerDetails(paired);
    await waitForText(app.browser, "Last successful sync");
    expect(await details.getText()).toContain("Device name is self-reported");

    for (const label of [
      "Sync now",
      "Unpair",
      "Revoke pairing…",
    ]) {
      const action = await details.$(`button=${label}`);
      expect(await action.isDisplayed(), label).toBe(true);
      expect(await action.isEnabled(), label).toBe(true);
    }
  });

  test("can run a real sync from the browser", async () => {
    await openPeerDetails(paired);
    await clickButton(app.browser, "Sync now", {
      within: 'section[aria-label$=" details"]',
    });
    await waitForManualSync();

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
    const details = await openPeerDetails(paired);
    await clickButton(app.browser, "Sync now", {
      within: 'section[aria-label$=" details"]',
    });
    await waitForText(app.browser, "Sync failed", 45_000);
    const status = await details.$('[data-slot="device-status"]');
    expect(await status.getAttribute("data-tone")).toBe("danger");

    await other.restart();
    await clickButton(app.browser, "Sync now", {
      within: 'section[aria-label$=" details"]',
    });
    await app.browser.waitUntil(
      async () => (await status.getAttribute("data-tone")) === "ready",
      {
        timeout: 45_000,
        timeoutMsg: "the browser did not show a new post-reconnect sync state",
      },
    );
    expectNoRawError(await outerHtml(app.browser));
  });

  test("offers the irreversible revoke confirmation", async () => {
    await clickButton(app.browser, "Revoke pairing…", {
      within: 'section[aria-label$=" details"]',
    });

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
    await clickButton(app.browser, "Unpair", {
      within: 'section[aria-label$=" details"]',
    });
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
    await waitForRosterRemoval(paired.pairing_id, 16);
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
    await clickButton(app.browser, "Refresh");
    await openPeerDetails(peer);

    await clickButton(app.browser, "Revoke pairing…", {
      within: 'section[aria-label$=" details"]',
    });
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
    await waitForRosterRemoval(minted.pairing_id, 16);
  });
});
