import { existsSync, readdirSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";

import { execa } from "execa";
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import { startDaemon } from "../src/harness/daemon.js";
import { daemonBinary } from "../src/harness/env.js";
import {
  accessibleSurface,
  expectNoFilesystemPath,
  expectNoRawError,
  outerHtml,
} from "../src/harness/leaks.js";
import { sleep, track, type Child } from "../src/harness/process.js";
import { visibleText, waitForRows } from "../src/harness/ui.js";

let app: App;
let dataHome: string;

beforeAll(async () => {
  app = await startApp({ seed: ["something to show first"] });
  dataHome = app.daemon.dataHome;
  await waitForRows(app.browser, 1);

  // The failure under test is the transport one: the socket path is what
  // discloses the local username, and it is only in play once the daemon dies
  // under a running window.
  await app.daemon.stop();

  await app.browser.waitUntil(
    async () => (await visibleText(app.browser)).includes("service"),
    {
      timeout: 40_000,
      interval: 500,
      timeoutMsg: "the UI never reported the service as unreachable",
    },
  );
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

test("the offline state is reported to the user", async () => {
  const text = await visibleText(app.browser);
  expect(text.toLowerCase()).toMatch(/service/);
});

test("no user-facing string contains a filesystem path (INV-12)", async () => {
  expectNoFilesystemPath(await accessibleSurface(app.browser), dataHome);
});

test("the raw transport error is not rendered anywhere", async () => {
  expectNoRawError(await outerHtml(app.browser));
});

/**
 * `XDG_DATA_HOME` is the knob the harness turns; the daemon resolves a project
 * directory underneath it, and only the daemon knows where. Found rather than
 * recomputed, so a second copy of `directories`' rules cannot disagree with the
 * first.
 */
function dataDirOf(home: string): string {
  for (const entry of readdirSync(home, { recursive: true }) as string[]) {
    if (path.basename(entry) === "copypaste-v2.db") {
      return path.dirname(path.join(home, entry));
    }
  }
  throw new Error("the daemon did not create a database under its data home");
}

/**
 * The other shape of failure: not a transport that went away, but a daemon that
 * came up and found nothing it could serve.
 *
 * Driven through the CLI rather than the window because the property under test
 * is the *wire* one — that the daemon holds its socket and answers with a
 * condition of its own, instead of exiting and leaving every client to conclude
 * "not running" and offer to start it again. What the window then draws is
 * `HistoryView.test.tsx`'s subject.
 */
describe("a startup failure no retry can clear", () => {
  test("is answered over the socket, and its sentence names no path", async () => {
    const daemon = await startDaemon();
    let halted: Child | undefined;
    try {
      await daemon.add("something worth protecting");
      const dataDir = dataDirOf(daemon.dataHome);
      await daemon.kill();

      // Present and unusable: `read_secret` rejects a wrong-length entry as
      // `KeystoreEntryUnusable`, which is the condition that no retry, restart
      // or reinstall changes.
      writeFileSync(path.join(dataDir, "device_secret.key"), Buffer.alloc(7));

      halted = track(
        execa(daemonBinary(), ["--foreground", "--port", String(daemon.peerPort)], {
          env: { ...process.env, ...daemon.env },
          stdio: ["ignore", "pipe", "pipe"],
          reject: false,
        }),
      );

      // Exit 1 is "cannot reach the daemon", which is what the socket looks
      // like for the moment before it is bound. Anything else is an answer.
      let result = await daemon.cli(["status"]);
      const deadline = Date.now() + 30_000;
      while (result.exitCode === 1 && Date.now() < deadline) {
        await sleep(250);
        result = await daemon.cli(["status"]);
      }

      expect(halted.exited(), "the daemon exited instead of holding the socket").toBe(
        false,
      );
      // Answered, not unreachable. The distinction is the whole point: a client
      // told "unreachable" offers to start a daemon that is already there.
      expect(result.exitCode).toBe(3);

      const message = `${result.stdout}\n${result.stderr}`;
      expect(message).toMatch(/cannot be used/i);
      // The honest half: saying so is what stops a client offering a retry.
      expect(message).toMatch(/will not change/i);
      expectNoFilesystemPath(message, daemon.dataHome);
    } finally {
      await halted?.stop();
      await daemon.stop();
    }
  }, 120_000);
});

/**
 * CLAUDE.md rule 3's second obligation, end to end: a daemon that meets a
 * CopyPaste 0.4 history **starts anyway** and **says so**.
 *
 * Both halves are the test. Blocking would take a working v2 away from a user
 * who is content to start fresh; staying silent is the upgrade the post-merge
 * review found — a fresh empty history and no explanation, which reads as data
 * loss.
 */
describe("a CopyPaste 0.4 history on the device", () => {
  test("is found, reported, and left exactly as it was", async () => {
    const daemon = await startDaemon();
    try {
      const dataDir = dataDirOf(daemon.dataHome);
      const legacy = path.join(dataDir, "clipboard.db");

      // An encrypted v0.4 file cannot be staged — v2 cannot derive v1's key —
      // so stage the one thing that is observable about one: SQLCipher-shaped
      // bytes under v0.4's filename, which is the branch
      // `copypaste_core::storage::legacy` identifies by name.
      writeFileSync(legacy, Buffer.alloc(3 * 4096, 0x5a));
      const before = statSync(legacy);

      await daemon.restart();

      const status = await daemon.cli(["status"]);
      expect(status.exitCode).toBe(0);
      expect(status.stdout).toMatch(/CopyPaste 0\.4/);
      expect(status.stdout).toMatch(/not changed it/);
      expectNoFilesystemPath(status.stdout, daemon.dataHome);

      // A v0.4 history is a notice, not a refusal: v2 still works.
      await daemon.add("captured after the upgrade");
      expect((await daemon.items()).length).toBeGreaterThan(0);

      // And the old file is as it was — not grown, not restamped, and with no
      // journal or WAL beside it. A downgrade has to find it intact.
      const after = statSync(legacy);
      expect(after.size).toBe(before.size);
      expect(after.mtimeMs).toBe(before.mtimeMs);
      expect(existsSync(`${legacy}-wal`)).toBe(false);
      expect(existsSync(`${legacy}-journal`)).toBe(false);
    } finally {
      await daemon.stop();
    }
  }, 120_000);
});
