import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import { execa } from "execa";
import { describe, expect, it } from "vitest";

import { describeSessionFailure } from "../src/harness/app.js";
import { startDaemon } from "../src/harness/daemon.js";
import { track } from "../src/harness/process.js";
import { recordRunEnvironment } from "../src/harness/run-manifest.js";
import {
  assertTauriBridge,
  assertTauriBrowserName,
} from "../src/harness/webview-guard.js";
import {
  assertMajorCompatibility,
  probeTauriSession,
  probeWindowsEnvironment,
} from "../src/harness/windows-environment.js";

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

describe("Windows environment probe", () => {
  it("accepts a different standalone Edge with matching WebView2 and EdgeDriver builds", () => {
    expect(() =>
      assertMajorCompatibility({
        edge: "151.0.4129.40",
        webview2: "131.0.2903.86",
        edgeDriver: "131.0.2903.86",
      }),
    ).not.toThrow();
  });

  it("rejects a WebView2 Runtime and EdgeDriver build mismatch", () => {
    expect(() =>
      assertMajorCompatibility({
        edge: "151.0.4129.40",
        webview2: "131.0.2903.86",
        edgeDriver: "131.0.2904.1",
      }),
    ).toThrow(/first-three-part versions are incompatible/);
  });

  it("records compatible versions before the session probe", async () => {
    const directory = mkdtempSync(path.join(os.tmpdir(), "cp-windows-probe-test-"));
    const manifest = path.join(directory, "run.log");
    try {
      await probeWindowsEnvironment({
        manifest,
        powershell: async () => "Microsoft Edge 151.0.4129.40",
        driverVersion: async () => "Microsoft Edge WebDriver 131.0.2903.86",
        webview2Version: "131.0.2903.86",
      });
      expect(readFileSync(manifest, "utf8")).toContain(
        "windowsEnvironmentProbe=ready",
      );
      expect(readFileSync(manifest, "utf8")).toContain("edgeVersion=151.0.4129.40");
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });

  it("classifies a failed compatibility probe as an environment failure", async () => {
    const directory = mkdtempSync(path.join(os.tmpdir(), "cp-windows-probe-test-"));
    const manifest = path.join(directory, "run.log");
    try {
      const failure = await probeWindowsEnvironment({
        manifest,
        powershell: async () => "Edge 151.0.4129.40",
        driverVersion: async () => "Microsoft Edge WebDriver 131.0.2904.1",
        webview2Version: "131.0.2903.86",
      }).then(() => undefined, (error: Error) => error);
      expect(failure?.name).toBe("WindowsEnvironmentProbeFailure");
      expect(failure?.message).toContain("before the native E2E suite");
      expect(readFileSync(manifest, "utf8")).toContain(
        "windowsEnvironmentProbe=failed",
      );
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });

  it("closes the app after a ready-session probe", async () => {
    const directory = mkdtempSync(path.join(os.tmpdir(), "cp-windows-probe-test-"));
    let stopped = false;
    try {
      await probeTauriSession(
        async () => ({
          async stop() {
            stopped = true;
          },
        }),
        path.join(directory, "run.log"),
      );
      expect(stopped).toBe(true);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });

  it("fails closed when session cleanup rejects after readiness", async () => {
    const directory = mkdtempSync(path.join(os.tmpdir(), "cp-windows-probe-test-"));
    const manifest = path.join(directory, "run.log");
    try {
      const failure = await probeTauriSession(
        async () => ({
          async stop() {
            throw new Error("driver cleanup failed");
          },
        }),
        manifest,
      ).then(() => undefined, (error: Error) => error);
      const written = readFileSync(manifest, "utf8");
      expect(failure?.name).toBe("WindowsEnvironmentProbeFailure");
      expect(failure?.message).toContain("during Tauri session cleanup");
      expect(written).toContain("tauriSessionProbe=ready");
      expect(written).toContain("tauriSessionProbe=failed");
      expect(written).toContain("tauriSessionProbeFailure=cleanup");
      expect(written).toContain("tauriSessionProbeCleanupError=driver cleanup failed");
      expect(written.lastIndexOf("tauriSessionProbe=failed")).toBeGreaterThan(
        written.lastIndexOf("tauriSessionProbe=ready"),
      );
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });
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
      assertTauriBridge({ bridge: false, url: "http://127.0.0.1:1420/" }, false),
    ).toThrow(/no IPC is under test/);
  });
});

/**
 * This is not the cold-start latency itself, which no warm runner can be made
 * to reproduce. It pins the evidence DMY-54 lacked: a reader must be able to
 * tell an exhausted budget from a crash without rerunning the job.
 */
describe("WebDriver session failure diagnostics", () => {
  const failure = () =>
    describeSessionFailure({
      budgetMs: 120_000,
      elapsedMs: 120_004,
      driverState: "pid=4242 state=running exitCode=none signal=none",
      driverLog: "hyper::Error(IncompleteMessage)\n",
      logPath: "/tmp/cp-e2e/logs/run-abc-driver.log",
    });

  it("states the elapsed time against the budget before the log", () => {
    const message = failure();
    expect(message).toContain("gave up after 120004ms of a 120000ms budget");
    expect(message.indexOf("120000ms budget")).toBeLessThan(
      message.indexOf("hyper::Error"),
    );
  });

  it("names the driver's state and where its full output was kept", () => {
    const message = failure();
    expect(message).toContain("pid=4242 state=running");
    expect(message).toContain("/tmp/cp-e2e/logs/run-abc-driver.log");
  });

  it("says so when the driver printed nothing at all", () => {
    expect(
      describeSessionFailure({
        budgetMs: 120_000,
        elapsedMs: 1_200,
        driverState: "pid=1 state=exited exitCode=1 signal=none",
        driverLog: "   \n",
        logPath: "/tmp/x.log",
      }),
    ).toContain("<no output captured>");
  });
});

/**
 * Run 31379514744 uploaded nothing at all, because the only file under the log
 * root was written after the step that died. The environment has to be on disk
 * before anything that can fail, and the failure has to end up beside it.
 */
describe("run manifest", () => {
  const inTemporaryDirectory = async (
    body: (manifest: string) => Promise<void>,
  ) => {
    const directory = mkdtempSync(path.join(os.tmpdir(), "cp-manifest-test-"));
    try {
      await body(path.join(directory, "run.log"));
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  };

  it("keeps the environment and the reason when the probe fails", async () => {
    await inTemporaryDirectory(async (manifest) => {
      const failure = await recordRunEnvironment({
        path: manifest,
        probe: () => Promise.reject(new Error("Add-Type : assembly not found")),
      }).then(
        () => undefined,
        (error: Error) => error,
      );

      expect(failure?.message).toMatch(/could not load System.Windows.Forms/);
      const written = readFileSync(manifest, "utf8");
      expect(written).toMatch(/^platform=/);
      expect(written).toMatch(/powershellWinFormsColdStartFailedMs=\d+/);
      expect(written).toContain("Add-Type : assembly not found");
    });
  });

  it("records what a successful probe cost", async () => {
    await inTemporaryDirectory(async (manifest) => {
      await recordRunEnvironment({ path: manifest, probe: async () => undefined });

      expect(readFileSync(manifest, "utf8")).toMatch(
        /powershellWinFormsColdStartMs=\d+/,
      );
    });
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
