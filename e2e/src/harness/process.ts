import { appendFileSync } from "node:fs";

import type { ResultPromise } from "execa";

/**
 * A long-running child plus the two things the harness needs from it that
 * execa's promise type does not expose: whether it has already exited, and
 * everything it has printed (which is the only diagnostic available when a
 * driver or a daemon dies during startup).
 */
export interface Child {
  readonly proc: ResultPromise;
  exited(): boolean;
  diagnostics(): string;
  log(): string;
  stop(): Promise<void>;
}

/** setTimeout that does not by itself keep the process alive. */
export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    const timer = setTimeout(resolve, ms);
    timer.unref?.();
  });
}

export function track(proc: ResultPromise, logPath?: string): Child {
  let done = false;
  let exitCode: number | "none" = "none";
  let exitSignal = "none";
  const lines: string[] = [];

  const record = (chunk: Buffer): void => {
    const text = chunk.toString();
    lines.push(text);
    if (logPath) appendFileSync(logPath, text);
  };

  proc.stdout?.on("data", record);
  proc.stderr?.on("data", record);
  void proc
    .then(
      (result) => {
        exitCode = result.exitCode ?? "none";
        exitSignal = result.signal ?? "none";
      },
      () => undefined,
    )
    .finally(() => {
      done = true;
      if (logPath) appendFileSync(logPath, `\n[harness] ${diagnostics()}\n`);
    });

  function signal(name: "SIGTERM" | "SIGKILL"): void {
    proc.kill(name);
  }

  function diagnostics(): string {
    const state = done ? "exited" : "running";
    return (
      `pid=${proc.pid ?? "unknown"} state=${state} ` +
      `exitCode=${exitCode} signal=${exitSignal}`
    );
  }

  return {
    proc,
    exited: () => done,
    diagnostics,
    log: () => lines.join(""),
    async stop() {
      if (done) return;
      signal("SIGTERM");
      await Promise.race([proc.catch(() => undefined), sleep(5_000)]);
      signal("SIGKILL");
    },
  };
}
