import path from "node:path";

import { execa } from "execa";
import waitOn from "wait-on";

import {
  DEV_SERVER_PORT,
  DEV_SERVER_URL,
  UI_DIR,
  requireDisplay,
  runLogPath,
} from "./env.js";
import { sleep, track, type Child } from "./process.js";
import { recordRunEnvironment } from "./run-manifest.js";

let server: Child | undefined;

export async function setup(): Promise<void> {
  await recordRunEnvironment();
  requireDisplay();
  await assertPortFree();

  const vite = path.join(UI_DIR, "node_modules/vite/bin/vite.js");

  // The vite binary directly, not `npm run dev`: killing npm leaves the vite
  // grandchild holding port 1420, and the next run then silently tests
  // whatever tree that orphan was started from.
  server = track(
    execa(process.execPath, [vite, "--port", String(DEV_SERVER_PORT)], {
      cwd: UI_DIR,
      stdio: ["ignore", "pipe", "pipe"],
      reject: false,
      killDescendants: true,
    }),
    runLogPath("dev-server.log"),
  );

  try {
    await waitOn({ resources: [DEV_SERVER_URL], timeout: 120_000, interval: 250 });
  } catch {
    throw new Error(
      `the Vite dev server never came up on ${DEV_SERVER_URL}:\n${server.log()}`,
    );
  }

  if (server.exited()) {
    throw new Error(
      `the dev server exited but ${DEV_SERVER_URL} still answers, so something ` +
        `else is serving it and the suite would test the wrong tree:\n${server.log()}`,
    );
  }

  await warmEntrypoint(server);
}

/**
 * `strictPort` is set in the UI's vite.config, so a dev server left behind by
 * an earlier run does not lose the port — it keeps it, our own server dies, and
 * every assertion afterwards runs against whatever tree that stale process was
 * started from. Refusing to start is the only way that failure is visible.
 */
async function assertPortFree(): Promise<void> {
  try {
    const response = await fetch(DEV_SERVER_URL, { signal: AbortSignal.timeout(3_000) });
    throw new Error(
      `something is already serving ${DEV_SERVER_URL} (HTTP ${response.status}). ` +
        `Stop it first — with strictPort set, this suite cannot tell its own dev ` +
        `server apart from a stale one.`,
    );
  } catch (error) {
    if (error instanceof Error && error.message.startsWith("something is already")) {
      throw error;
    }
  }
}

/**
 * The root URL answers before Vite has pre-bundled dependencies, and a page
 * that loads during that window gets a module graph that fails to import —
 * which looks exactly like an app that is broken. Pulling the entrypoint until
 * it is really served makes the wait deterministic.
 */
async function warmEntrypoint(server: Child): Promise<void> {
  const deadline = Date.now() + 180_000;
  for (;;) {
    try {
      const response = await fetch(`${DEV_SERVER_URL}/src/main.tsx`, {
        signal: AbortSignal.timeout(10_000),
      });
      if (response.ok) {
        const body = await response.text();
        if (body.length > 0) return;
      }
    } catch {
      /* still optimizing */
    }
    if (Date.now() > deadline) {
      throw new Error(
        `the dev server never served /src/main.tsx:\n${server.log()}`,
      );
    }
    await sleep(500);
  }
}

export async function teardown(): Promise<void> {
  await server?.stop();
}
