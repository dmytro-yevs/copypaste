import { createConnection } from "node:net";

import { sleep } from "./process.js";

const PORT_PROBE_BUDGET_MS = 500;
const PORT_CLOSE_BUDGET_MS = 10_000;

export async function waitForPortsClosed(
  ports: readonly number[],
  budgetMs = PORT_CLOSE_BUDGET_MS,
): Promise<void> {
  if (ports.length === 0) return;
  const deadline = Date.now() + budgetMs;
  for (;;) {
    const remainingMs = deadline - Date.now();
    if (remainingMs <= 0) break;
    const open = await Promise.all(
      ports.map((port) =>
        probePort(port, Math.min(PORT_PROBE_BUDGET_MS, remainingMs)),
      ),
    );
    if (!open.some(Boolean)) return;
    if (Date.now() >= deadline) break;
    await sleep(Math.min(100, deadline - Date.now()));
  }
  throw new Error(
    `WebDriver ports remain open after ${budgetMs}ms: ${ports.join(", ")}`,
  );
}

function probePort(port: number, timeoutMs: number): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = createConnection({ host: "127.0.0.1", port });
    let settled = false;
    const finish = (open: boolean): void => {
      if (settled) return;
      settled = true;
      socket.destroy();
      resolve(open);
    };

    socket.once("connect", () => finish(true));
    socket.once("timeout", () => finish(true));
    socket.once("error", (error: NodeJS.ErrnoException) => {
      finish(error.code !== "ECONNREFUSED");
    });
    socket.setTimeout(timeoutMs);
  });
}

export function createIdempotentStop(
  stop: () => Promise<void>,
): () => Promise<void> {
  let stopPromise: Promise<void> | undefined;
  return () => {
    if (stopPromise) return stopPromise;
    stopPromise = Promise.resolve().then(stop);
    return stopPromise;
  };
}

export function aggregateErrors(
  primary: unknown,
  ...additional: unknown[]
): AggregateError {
  const errors = [primary, ...additional];
  return new AggregateError(
    errors,
    errors
      .map((error) => (error instanceof Error ? error.message : String(error)))
      .join("\n"),
  );
}

export async function runCleanup(
  ...cleanups: Array<() => Promise<void>>
): Promise<unknown> {
  const errors: unknown[] = [];
  for (const cleanup of cleanups) {
    try {
      await cleanup();
    } catch (error) {
      errors.push(error);
    }
  }
  if (errors.length === 0) return undefined;
  return errors.length === 1
    ? errors[0]
    : aggregateErrors(errors[0], ...errors.slice(1));
}
