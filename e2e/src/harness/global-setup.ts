import { execa, type ResultPromise } from "execa";
import waitOn from "wait-on";

import { DEV_SERVER_PORT, DEV_SERVER_URL, UI_DIR, requireDisplay } from "./env.js";

let server: ResultPromise | undefined;

export async function setup(): Promise<void> {
  requireDisplay();

  server = execa("npm", ["run", "dev", "--", "--port", String(DEV_SERVER_PORT)], {
    cwd: UI_DIR,
    stdio: ["ignore", "pipe", "pipe"],
    reject: false,
  });
  const log: string[] = [];
  server.stdout?.on("data", (c: Buffer) => log.push(c.toString()));
  server.stderr?.on("data", (c: Buffer) => log.push(c.toString()));

  try {
    await waitOn({ resources: [DEV_SERVER_URL], timeout: 120_000, interval: 250 });
  } catch {
    throw new Error(
      `the Vite dev server never came up on ${DEV_SERVER_URL}:\n${log.join("")}`,
    );
  }

  await warmEntrypoint(log);
}

/**
 * The root URL answers before Vite has pre-bundled dependencies, and a page
 * that loads during that window gets a module graph that fails to import —
 * which looks exactly like an app that is broken. Pulling the entrypoint until
 * it is really served makes the wait deterministic.
 */
async function warmEntrypoint(log: string[]): Promise<void> {
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
        `the dev server never served /src/main.tsx:\n${log.join("")}`,
      );
    }
    await new Promise((r) => setTimeout(r, 500));
  }
}

export async function teardown(): Promise<void> {
  server?.kill("SIGTERM");
  await Promise.race([
    server?.catch(() => undefined) ?? Promise.resolve(),
    new Promise((r) => setTimeout(r, 5_000)),
  ]);
  server?.kill("SIGKILL");
}
