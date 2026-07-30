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

interface Options {
  /**
   * Signal the whole process group rather than the one process. Required for
   * `tauri-driver`, which spawns WebKitWebDriver and the app as children:
   * signalling only the parent orphans both, and an orphaned WebKitWebDriver
   * holds the single session slot and keeps the runner's event loop alive.
   * The spawn must pass `detached: true` for this to have a group to signal.
   */
  group?: boolean;
}

/** setTimeout that does not by itself keep the process alive. */
export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    const timer = setTimeout(resolve, ms);
    timer.unref?.();
  });
}

export function track(proc: ResultPromise, options: Options = {}): Child {
  let done = false;
  const lines: string[] = [];

  proc.stdout?.on("data", (chunk: Buffer) => lines.push(chunk.toString()));
  proc.stderr?.on("data", (chunk: Buffer) => lines.push(chunk.toString()));
  void proc.catch(() => undefined).finally(() => {
    done = true;
  });

  function signal(name: "SIGTERM" | "SIGKILL"): void {
    const { pid } = proc;
    if (options.group && pid !== undefined) {
      try {
        process.kill(-pid, name);
        return;
      } catch {
        /* the group is already gone */
      }
    }
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
