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

export function track(proc: ResultPromise): Child {
  let done = false;
  const lines: string[] = [];

  proc.stdout?.on("data", (chunk: Buffer) => lines.push(chunk.toString()));
  proc.stderr?.on("data", (chunk: Buffer) => lines.push(chunk.toString()));
  void proc.catch(() => undefined).finally(() => {
    done = true;
  });

  function signal(name: "SIGTERM" | "SIGKILL"): void {
    proc.kill(name);
  }

  return {
    proc,
    exited: () => done,
    log: () => lines.join(""),
    async stop() {
      if (done) return;
      signal("SIGTERM");
      await Promise.race([proc.catch(() => undefined), sleep(5_000)]);
      signal("SIGKILL");
    },
  };
}
