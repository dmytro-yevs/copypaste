import { mkdtempSync, mkdirSync, rmSync } from "node:fs";
import path from "node:path";
import { execa, type ResultPromise } from "execa";

import { RUN_ROOT, cliBinary, daemonBinary, freePort } from "./env.js";

export interface Daemon {
  /** Injected into the app process so the bridge finds this daemon's socket. */
  readonly env: Record<string, string>;
  readonly dataHome: string;
  add(content: string): Promise<void>;
  addMany(contents: readonly string[]): Promise<void>;
  itemCount(): Promise<number>;
  stop(): Promise<void>;
}

interface CliItem {
  id: string;
  content: string;
  is_sensitive: boolean;
}

export async function startDaemon(): Promise<Daemon> {
  mkdirSync(RUN_ROOT, { recursive: true });
  const dataHome = mkdtempSync(path.join(RUN_ROOT, "run-"));
  // The daemon and the CLI both resolve their directory through `directories`,
  // so XDG_DATA_HOME is the single knob that isolates a run from the developer's
  // real history and from other runs.
  const env = { ...process.env, XDG_DATA_HOME: dataHome } as Record<string, string>;

  // Peer listener port: the default is fixed, so two runs on one host collide.
  const peerPort = await freePort();

  const proc = execa(daemonBinary(), ["--foreground", "--port", String(peerPort)], {
    env,
    stdio: ["ignore", "pipe", "pipe"],
    reject: false,
  });

  const log: string[] = [];
  proc.stdout?.on("data", (c: Buffer) => log.push(c.toString()));
  proc.stderr?.on("data", (c: Buffer) => log.push(c.toString()));

  const cli = (args: string[]) =>
    execa(cliBinary(), args, { env, reject: false, timeout: 20_000 });

  const deadline = Date.now() + 30_000;
  for (;;) {
    const probe = await cli(["--json", "status"]);
    if (probe.exitCode === 0) break;
    if (proc.exitCode !== null && proc.exitCode !== undefined) {
      throw new Error(`daemon exited early (${proc.exitCode}):\n${log.join("")}`);
    }
    if (Date.now() > deadline) {
      throw new Error(`daemon never became reachable:\n${log.join("")}`);
    }
    await new Promise((r) => setTimeout(r, 250));
  }

  async function add(content: string): Promise<void> {
    const result = await cli(["add", content]);
    if (result.exitCode !== 0) {
      throw new Error(`\`copypaste add\` failed: ${result.stderr || result.stdout}`);
    }
  }

  return {
    env: { XDG_DATA_HOME: dataHome },
    dataHome,
    add,
    async addMany(contents) {
      for (const content of contents) await add(content);
    },
    async itemCount() {
      const result = await cli(["--json", "list"]);
      if (result.exitCode !== 0) throw new Error(`\`copypaste list\` failed`);
      const parsed = JSON.parse(result.stdout) as { data: CliItem[] };
      return parsed.data.length;
    },
    async stop() {
      proc.kill("SIGTERM");
      await Promise.race([
        proc.catch(() => undefined),
        new Promise((r) => setTimeout(r, 5_000)),
      ]);
      proc.kill("SIGKILL");
      rmSync(dataHome, { recursive: true, force: true });
    },
  };
}
