import { mkdtempSync, mkdirSync, rmSync } from "node:fs";
import path from "node:path";
import { execa } from "execa";

import { RUN_ROOT, cliBinary, daemonBinary, freePort } from "./env.js";
import { track } from "./process.js";

export interface Daemon {
  /** Injected into the app process so the bridge finds this daemon's socket. */
  readonly env: Record<string, string>;
  readonly dataHome: string;
  add(content: string): Promise<void>;
  addMany(contents: readonly string[]): Promise<void>;
  items(): Promise<CliItem[]>;
  remove(id: string): Promise<void>;
  removeMany(ids: readonly string[]): Promise<void>;
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

  const child = track(
    execa(daemonBinary(), ["--foreground", "--port", String(peerPort)], {
      env,
      stdio: ["ignore", "pipe", "pipe"],
      reject: false,
    }),
  );

  const cli = (args: string[]) =>
    execa(cliBinary(), args, { env, reject: false, timeout: 20_000 });

  const deadline = Date.now() + 30_000;
  for (;;) {
    const probe = await cli(["--json", "status"]);
    if (probe.exitCode === 0) break;
    if (child.exited()) {
      throw new Error(`the daemon exited during startup:\n${child.log()}`);
    }
    if (Date.now() > deadline) {
      throw new Error(`the daemon never became reachable:\n${child.log()}`);
    }
    await new Promise((r) => setTimeout(r, 250));
  }

  let stopped = false;

  async function add(content: string): Promise<void> {
    const result = await cli(["add", content]);
    if (result.exitCode !== 0) {
      throw new Error(`\`copypaste add\` failed: ${result.stderr || result.stdout}`);
    }
  }

  async function removeOne(id: string): Promise<void> {
    const result = await cli(["delete", id]);
    if (result.exitCode !== 0) {
      throw new Error(`\`copypaste delete\` failed: ${result.stderr || result.stdout}`);
    }
  }

  return {
    env: { XDG_DATA_HOME: dataHome },
    dataHome,
    add,
    async addMany(contents) {
      for (const content of contents) await add(content);
    },
    async items() {
      // `list` defaults to 50; the UI asks for PAGE_SIZE (200), so anything
      // less here silently disagrees with what is on screen.
      const result = await cli(["--json", "list", "--limit", "1000"]);
      if (result.exitCode !== 0) throw new Error(`\`copypaste list\` failed`);
      return (JSON.parse(result.stdout) as { data: CliItem[] }).data;
    },
    async remove(id) {
      await removeOne(id);
    },
    async removeMany(ids) {
      // Each call is a process spawn against a debug binary; sequentially this
      // takes longer than the UI's poll interval, which makes "the list
      // shrank" impossible to observe as one event.
      for (let i = 0; i < ids.length; i += 8) {
        await Promise.all(ids.slice(i, i + 8).map(removeOne));
      }
    },
    async stop() {
      // Tests stop the daemon themselves to produce the offline state, and the
      // app teardown stops it again.
      if (stopped) return;
      stopped = true;
      await child.stop();
      rmSync(dataHome, { recursive: true, force: true });
    },
  };
}
