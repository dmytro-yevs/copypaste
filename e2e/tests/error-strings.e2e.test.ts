import { readdirSync, writeFileSync } from "node:fs";
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
import { waitForRows } from "../src/harness/ui.js";

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
    async () => (await accessibleSurface(app.browser)).includes("Background service unreachable"),
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
  const surface = await accessibleSurface(app.browser);
  expect(surface).toContain("Background service unreachable");
});

test("no user-facing string contains a filesystem path (INV-12)", async () => {
  expectNoFilesystemPath(await accessibleSurface(app.browser), dataHome);
});

test("the raw transport error is not rendered anywhere", async () => {
  expectNoRawError(await outerHtml(app.browser));
});

/**
 * The harness relocates the complete data directory, and only the daemon knows
 * which files it creates there. Found rather than recomputed, so the test does
 * not grow a second copy of the storage layout.
 */
function dataDirOf(home: string): string {
  for (const entry of readdirSync(home, { recursive: true }) as string[]) {
    if (path.basename(entry) === "copypaste-v2.db") {
      return path.dirname(path.join(home, entry));
    }
  }
  throw new Error("the daemon did not create a database under its data home");
}

async function writeUnusableSecret(dataDir: string): Promise<void> {
  if (process.platform !== "win32") {
    writeFileSync(path.join(dataDir, "device_secret.key"), Buffer.alloc(7));
    return;
  }

  await execa(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      "Add-Type -AssemblyName System.Security; " +
        "$entropy = [Text.Encoding]::UTF8.GetBytes('copypaste/v2/device-secret/dpapi-entropy'); " +
        "$sealed = [Security.Cryptography.ProtectedData]::Protect([byte[]]::new(7), $entropy, [Security.Cryptography.DataProtectionScope]::CurrentUser); " +
        "[IO.File]::WriteAllBytes($env:COPYPASTE_E2E_SECRET_PATH, $sealed)",
    ],
    {
      env: {
        ...process.env,
        COPYPASTE_E2E_SECRET_PATH: path.join(dataDir, "device_secret.dpapi"),
      },
      timeout: 20_000,
    },
  );
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
      await writeUnusableSecret(dataDir);

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
