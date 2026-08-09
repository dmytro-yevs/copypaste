import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { execa } from "execa";

import { RUN_ROOT, cliBinary, daemonBinary, freePort } from "./env.js";
import { sleep, track, type Child } from "./process.js";

export interface Daemon {
  /** Injected into the app process so the bridge finds this daemon's socket. */
  readonly env: Record<string, string>;
  readonly dataHome: string;
  /** The TCP port the peer listener binds, so a second daemon can dial it. */
  readonly peerPort: number;
  /** The CLI, against this daemon. Raw: the caller decides what a failure means. */
  cli(args: readonly string[]): Promise<CliResult>;
  /** `--json` plus the envelope unwrapped, throwing on a non-zero exit. */
  json<T>(args: readonly string[]): Promise<T>;
  add(content: string): Promise<void>;
  addMany(contents: readonly string[]): Promise<void>;
  items(): Promise<CliItem[]>;
  remove(id: string): Promise<void>;
  removeMany(ids: readonly string[]): Promise<void>;
  /**
   * Stop the process and leave the data directory alone.
   *
   * Distinct from `stop`, which also deletes it: the offline tests need the
   * socket to go away while the database, the config and the peer store stay
   * where a restarted daemon — including one the app starts for itself — will
   * find them.
   */
  kill(): Promise<void>;
  /** Start it again on the same data directory and the same peer port. */
  restart(): Promise<void>;
  /** Move an unredeemed code past its deadline, preserving the native store format. */
  expirePairing(pairingId: string): Promise<void>;
  stop(): Promise<void>;
}

interface CliResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

interface CliItem {
  id: string;
  content: string;
  is_sensitive: boolean;
  pinned: boolean;
}

export async function startDaemon(): Promise<Daemon> {
  mkdirSync(RUN_ROOT, { recursive: true });
  const dataHome = mkdtempSync(path.join(RUN_ROOT, "run-"));
  const isolation: Record<string, string> =
    process.platform === "win32"
      ? {
          APPDATA: dataHome,
          COPYPASTE_SOCKET: path.join(dataHome, "daemon.sock"),
        }
      : { XDG_DATA_HOME: dataHome };
  const env = { ...process.env, ...isolation } as Record<string, string>;

  // Peer listener port: the default is fixed, so two runs on one host collide.
  const peerPort = await freePort();

  const cli = async (args: readonly string[]): Promise<CliResult> => {
    const result = await execa(cliBinary(), [...args], {
      env,
      reject: false,
      timeout: 20_000,
    });
    return {
      exitCode: result.exitCode ?? -1,
      stdout: result.stdout ?? "",
      stderr: result.stderr ?? "",
    };
  };

  let child: Child = await spawn();
  let stopped = false;

  async function spawn(): Promise<Child> {
    const proc = track(
      execa(daemonBinary(), ["--foreground", "--port", String(peerPort)], {
        env,
        stdio: ["ignore", "pipe", "pipe"],
        reject: false,
      }),
    );

    const deadline = Date.now() + 30_000;
    for (;;) {
      const probe = await cli(["--json", "status"]);
      if (probe.exitCode === 0) return proc;
      if (proc.exited()) {
        throw new Error(`the daemon exited during startup:\n${proc.log()}`);
      }
      if (Date.now() > deadline) {
        throw new Error(`the daemon never became reachable:\n${proc.log()}`);
      }
      await sleep(250);
    }
  }

  async function json<T>(args: readonly string[]): Promise<T> {
    const result = await cli(["--json", ...args]);
    if (result.exitCode !== 0) {
      throw new Error(
        `\`copypaste ${args.join(" ")}\` failed (${result.exitCode}): ` +
          `${result.stderr || result.stdout}`,
      );
    }
    const response = JSON.parse(result.stdout) as {
      data: Record<string, T>;
    };
    const payloads = Object.values(response.data);
    if (payloads.length !== 1) {
      throw new Error("the CLI returned an invalid typed payload");
    }
    return payloads[0]!;
  }

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

  function findPeerStore(directory: string): string | null {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const candidate = path.join(directory, entry.name);
      if (entry.isFile() && entry.name === "peers.json") return candidate;
      if (entry.isDirectory()) {
        const found = findPeerStore(candidate);
        if (found) return found;
      }
    }
    return null;
  }

  return {
    env: isolation,
    dataHome,
    peerPort,
    cli,
    json,
    add,
    async addMany(contents) {
      for (const content of contents) await add(content);
    },
    async items() {
      // `list` defaults to 50; the UI asks for PAGE_SIZE (200), so anything
      // less here silently disagrees with what is on screen.
      // `data` is a page envelope, not a bare array: it carries the count of
      // rows that would not decrypt alongside the ones that did
      // (`ItemPage.skipped_undecryptable`).
      const page = await json<{ items: CliItem[] }>(["list", "--limit", "1000"]);
      return page.items;
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
    async kill() {
      await child.stop();
    },
    async restart() {
      await child.stop();
      child = await spawn();
    },
    async expirePairing(pairingId) {
      await child.stop();
      const storePath = findPeerStore(dataHome);
      if (!storePath) throw new Error("the daemon did not create a peer store");
      const store = JSON.parse(readFileSync(storePath, "utf8")) as {
        pending?: Record<string, number>;
      };
      if (store.pending?.[pairingId] === undefined) {
        throw new Error("the requested pairing is not pending");
      }
      store.pending[pairingId] = 1;
      writeFileSync(storePath, JSON.stringify(store));
      child = await spawn();
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
