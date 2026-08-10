import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import { execa } from "execa";
import { describe, expect, it } from "vitest";

import {
  assertTauriBridge,
  assertTauriBrowserName,
} from "../src/harness/app.js";
import { startDaemon } from "../src/harness/daemon.js";
import { track } from "../src/harness/process.js";

describe("Tauri WebDriver capabilities", () => {
  it("accepts WebView2 only on Windows", () => {
    expect(() =>
      assertTauriBrowserName({ browserName: "webview2" }, "win32"),
    ).not.toThrow();
    expect(() => assertTauriBrowserName({ browserName: "wry" }, "win32")).toThrow(
      /expected the Tauri WebView \("webview2"\)/,
    );
  });

  it("retains the wry capability requirement off Windows", () => {
    expect(() =>
      assertTauriBrowserName({ browserName: "wry" }, "linux"),
    ).not.toThrow();
    expect(() => assertTauriBrowserName({ browserName: "webview2" }, "linux")).toThrow(
      /expected the Tauri WebView \("wry"\)/,
    );
  });

  it.each(["MicrosoftEdge", "chrome", undefined])(
    "rejects a browser-only session with browserName=%s",
    (browserName) => {
      expect(() => assertTauriBrowserName({ browserName }, "win32")).toThrow(
        /The session is not the app under test/,
      );
    },
  );
});

describe("Tauri bridge startup", () => {
  it("waits only for WebView2's initial about:blank", () => {
    expect(() =>
      assertTauriBridge({ bridge: false, url: "about:blank" }, false),
    ).not.toThrow();
    expect(() =>
      assertTauriBridge({ bridge: false, url: "about:blank" }, true),
    ).toThrow(/no IPC is under test/);
  });

  it("rejects another bridgeless page immediately", () => {
    expect(() =>
      assertTauriBridge({ bridge: false, url: "http://localhost:1420/" }, false),
    ).toThrow(/no IPC is under test/);
  });
});

describe("child-process diagnostics", () => {
  it("preserves output and the exit result in the uploaded log", async () => {
    const directory = mkdtempSync(path.join(os.tmpdir(), "cp-process-test-"));
    const logPath = path.join(directory, "child.log");
    try {
      const child = track(
        execa(
          process.execPath,
          ["-e", "console.error('diagnostic marker'); process.exit(7)"],
          { reject: false },
        ),
        logPath,
      );

      await child.proc;
      await Promise.resolve();

      expect(child.exited()).toBe(true);
      expect(child.diagnostics()).toMatch(/state=exited exitCode=7/);
      expect(child.log()).toContain("diagnostic marker");
      expect(readFileSync(logPath, "utf8")).toMatch(
        /diagnostic marker[\s\S]*state=exited exitCode=7/,
      );
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });
});

describe("daemon bulk-delete harness", () => {
  it("never overlaps real CLI delete processes", async () => {
    let activeDeletes = 0;
    let maxActiveDeletes = 0;
    let settledDeletes = 0;
    const daemon = await startDaemon((args, phase) => {
      if (args[0] !== "delete") return;
      if (phase === "started") {
        activeDeletes += 1;
        maxActiveDeletes = Math.max(maxActiveDeletes, activeDeletes);
      } else {
        activeDeletes -= 1;
        settledDeletes += 1;
      }
    });

    try {
      const contents = Array.from(
        { length: 12 },
        (_, index) => `serial bulk-delete item ${index}`,
      );
      await daemon.addMany(contents);
      const items = (await daemon.items()).filter((item) =>
        item.content.startsWith("serial bulk-delete item "),
      );
      expect(items).toHaveLength(contents.length);

      await daemon.removeMany(items.map((item) => item.id));

      expect({ activeDeletes, maxActiveDeletes, settledDeletes }).toEqual({
        activeDeletes: 0,
        maxActiveDeletes: 1,
        settledDeletes: items.length,
      });
      const remainingIds = new Set((await daemon.items()).map((item) => item.id));
      expect(items.some((item) => remainingIds.has(item.id))).toBe(false);
    } finally {
      await daemon.stop();
    }
  }, 120_000);
});
