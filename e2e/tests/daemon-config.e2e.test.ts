/**
 * `GetConfig` / `SetConfig`, against a real daemon over a real socket.
 *
 * **The finding this file recorded is closed.** `get_config` and `set_config`
 * are routed Tauri commands, and Settings has a Service tab built on them, so
 * the two properties a screen is built on are now asserted at the screen:
 * `crates/copypaste-ui/src/components/settings/ServiceTab.test.tsx` covers the
 * value that round-trips and the patch that carries only the field it names,
 * including the restart badge on `lan_visibility`.
 *
 * What stays here is the half a jsdom test cannot reach: a rejection that
 * changes *nothing* on a real daemon, and the concurrent-writer race below,
 * which is a defect in `daemon/src/settings.rs` rather than anything a client
 * can do about. `poll_interval_ms` is set in the first block and read again in
 * the last, so the round trip earns its place twice over.
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { startDaemon, type Daemon } from "../src/harness/daemon.js";
import { expectNoFilesystemPath } from "../src/harness/leaks.js";

interface ConfigData {
  poll_interval_ms: number;
  history_limit: number;
  dedup_window_secs: number;
  lan_visibility: boolean;
  sync_enabled: boolean;
}

interface ConfigApplied {
  config: ConfigData;
  restart_required: string[];
}

let daemon: Daemon;

beforeAll(async () => {
  daemon = await startDaemon();
}, 120_000);

afterAll(async () => {
  await daemon?.stop();
});

const show = () => daemon.json<ConfigApplied>(["config", "show"]);

describe("the round trip", () => {
  test("a setting that is written comes back", async () => {
    const before = await show();
    expect(before.config.poll_interval_ms).toBeGreaterThan(0);
    expect(before.restart_required).toEqual([]);

    const applied = await daemon.json<ConfigApplied>([
      "config",
      "set",
      "--poll-interval-ms",
      "1500",
    ]);
    expect(applied.config.poll_interval_ms).toBe(1500);

    // Read back through a second process, so this is the daemon's answer and
    // not the writer's echo.
    expect((await show()).config.poll_interval_ms).toBe(1500);
  });

  test("a field that needs a restart says so; a live one does not", async () => {
    const restart = await daemon.json<ConfigApplied>([
      "config",
      "set",
      "--lan-visibility",
      "false",
    ]);
    expect(restart.restart_required).toContain("lan_visibility");

    const live = await daemon.json<ConfigApplied>([
      "config",
      "set",
      "--dedup-window-secs",
      "45",
    ]);
    expect(live.restart_required).toEqual([]);
    expect(live.config.dedup_window_secs).toBe(45);
  });
});

describe("a rejected value", () => {
  test("changes nothing, and its message names the bound rather than a file", async () => {
    const before = await show();

    const result = await daemon.cli(["--json", "config", "set", "--poll-interval-ms", "1"]);
    expect(result.exitCode).not.toBe(0);

    const message = `${result.stdout}\n${result.stderr}`;
    expect(message).toContain("poll_interval_ms");
    expect(message).toMatch(/between \d+ and \d+/);
    expectNoFilesystemPath(message, daemon.dataHome);

    expect((await show()).config).toEqual(before.config);
  });

  test("a rejected value in a batch does not let the others through", async () => {
    const before = await show();

    const result = await daemon.cli([
      "--json",
      "config",
      "set",
      "--history-limit",
      "4242",
      "--poll-interval-ms",
      "0",
    ]);
    expect(result.exitCode).not.toBe(0);
    expect((await show()).config.history_limit).toBe(before.config.history_limit);
  });
});

describe("concurrent writers", () => {
  /**
   * Regression guard for the lost-update defect fixed by d08b517d.
   * Concurrent clients patch different fields; `Settings::apply` must keep one
   * write lock across read, validation, persistence, and publication so no
   * writer can erase another's field. Several rounds make the race observable
   * if that lock scope regresses.
   */
  test("patches over different fields all survive", async () => {
    const lost: string[] = [];

    for (let round = 1; round <= 6; round += 1) {
      // Four writers rather than two: whether the race is *observed* depends on
      // how far apart the process spawns land, so the width is what makes this
      // more than a coin toss. Each field is written by exactly one writer, so
      // any disagreement afterwards is a lost update and nothing else.
      const want = {
        history_limit: 700 + round,
        retention_days: 10 + round,
        dedup_window_secs: 20 + round,
        max_text_size_bytes: 1_000_000 + round,
      };
      await Promise.all([
        daemon.json(["config", "set", "--history-limit", String(want.history_limit)]),
        daemon.json(["config", "set", "--retention-days", String(want.retention_days)]),
        daemon.json(["config", "set", "--dedup-window-secs", String(want.dedup_window_secs)]),
        daemon.json([
          "config",
          "set",
          "--max-text-size-bytes",
          String(want.max_text_size_bytes),
        ]),
      ]);

      const after = (await show()).config as unknown as Record<string, number>;
      for (const [field, expected] of Object.entries(want)) {
        if (after[field] !== expected) {
          lost.push(
            `round ${round}: wrote ${field}=${expected}, read back ${after[field]}`,
          );
        }
      }
    }

    expect(
      lost,
      "a concurrent SetConfig lost a field that the other patch did not name — " +
        "see daemon/src/settings.rs::apply, which is a read-modify-write that " +
        "does not hold one lock across the whole operation:\n" +
        lost.join("\n"),
    ).toEqual([]);
  });

  test("the field nobody wrote is untouched", async () => {
    expect((await show()).config.poll_interval_ms).toBe(1500);
  });
});
