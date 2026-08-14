import { describe, expect, test } from "vitest";

import { adbFailureText, classifyAdbFailure, isEmptyDeviceAnswer } from "../src/harness/adb-failure.js";
import { readinessSchedule, waitForReadiness, type Probe } from "../src/harness/readiness.js";

const noSleep = async (): Promise<void> => undefined;

describe("bounding the adb processes one wait may spend", () => {
  // Before/after, computed rather than asserted from memory: the fixed 1s poll
  // this replaced ran timeoutMs/1000 probes at up to 5 adb launches each.
  test("costs a fraction of the fixed poll it replaced", () => {
    for (const timeoutMs of [90_000, 150_000]) {
      const before = timeoutMs / 1_000;
      const after = readinessSchedule({
        timeoutMs,
        processBudget: 160,
        processesPerProbe: 5,
        maxDelayMs: 5_000,
      });
      expect(after.attempts).toBeLessThan(before / 3);
      expect(after.processes).toBeLessThanOrEqual(160);
    }
  });

  test("a budget that cannot pay for one probe is refused rather than rounded to zero", () => {
    expect(() => readinessSchedule({ timeoutMs: 30_000, processBudget: 3, processesPerProbe: 5 })).toThrow(
      /process budget of 3 cannot pay for one probe costing 5/,
    );
  });

  test("a schedule the budget truncated does not report itself as having waited the timeout", () => {
    const starved = readinessSchedule({ timeoutMs: 150_000, processBudget: 20, processesPerProbe: 5 });
    expect(starved.attempts).toBe(4);
    expect(starved.limitedBy).toBe("process-budget");

    const roomy = readinessSchedule({
      timeoutMs: 30_000,
      processBudget: 160,
      processesPerProbe: 5,
      maxDelayMs: 5_000,
    });
    expect(roomy.limitedBy).toBe("timeout");
  });
});

describe("what a probe outcome decides", () => {
  test("a condition that comes ready returns its observation", async () => {
    let probes = 0;
    const value = await waitForReadiness<string>({
      description: "a fixture that comes ready",
      timeoutMs: 30_000,
      processBudget: 40,
      processesPerProbe: 1,
      sleep: noSleep,
      probe: async () =>
        ++probes < 3 ? { kind: "not-ready", why: "still starting" } : { kind: "ready", value: "observed" },
    });
    expect(value).toBe("observed");
    expect(probes).toBe(3);
  });

  // The defect this module exists for: `device not found` was retried to the
  // deadline and then reported as "no process named … is running".
  test("an invariant propagates on the probe that found it, and is not retried", async () => {
    let probes = 0;
    await expect(
      waitForReadiness({
        description: "a fixture that cannot recover",
        timeoutMs: 150_000,
        processBudget: 40,
        processesPerProbe: 1,
        sleep: noSleep,
        probe: async () => {
          probes++;
          return { kind: "invariant", why: "device 'emulator-5554' not found" };
        },
      }),
    ).rejects.toThrow(/cannot become ready: device 'emulator-5554' not found/);
    expect(probes).toBe(1);
  });

  test("an exhausted budget names itself, the last outcome and the diagnostics", async () => {
    let probes = 0;
    await expect(
      waitForReadiness({
        description: "a fixture that never comes ready",
        timeoutMs: 150_000,
        processBudget: 6,
        processesPerProbe: 2,
        sleep: noSleep,
        diagnostics: async () => "fixture diagnostics",
        probe: async () => {
          probes++;
          return { kind: "transient", why: "adb.exe: protocol fault" };
        },
      }),
    ).rejects.toThrow(
      /process budget of 6 ran out.*transient: adb\.exe: protocol fault.*fixture diagnostics/s,
    );
    expect(probes).toBe(3);
  });

  test("a probe that returns nothing is refused rather than read as ready", async () => {
    await expect(
      waitForReadiness({
        description: "a fixture with a broken probe",
        timeoutMs: 1_000,
        processBudget: 40,
        processesPerProbe: 1,
        sleep: noSleep,
        probe: async () => undefined as unknown as Probe<string>,
      }),
    ).rejects.toThrow(/must return a ready, not-ready, transient or invariant Probe/);
  });
});

describe("reading an adb failure", () => {
  // Observed on adb 1.0.41 / platform-tools 36.0.0. The prefix is adb's argv[0]
  // for the shell service and `error:` for get-state, so a classifier keyed on
  // the prefix would miss half of them.
  test("a device that is not there cannot be waited for", () => {
    for (const stderr of [
      "adb.exe: device 'emulator-9999' not found",
      "error: device 'emulator-9999' not found",
      "adb.exe: no devices/emulators found",
      "error: device unauthorized.",
    ]) {
      expect(classifyAdbFailure({ exitCode: 1, stderr }).kind).toBe("invariant");
    }
  });

  test("an unrecognised failure is carried forward instead of being read as a device state", () => {
    expect(classifyAdbFailure({ exitCode: 1, stderr: "adb.exe: protocol fault (couldn't read status)" })).toEqual({
      kind: "transient",
      why: "adb.exe: protocol fault (couldn't read status) (adb exit 1)",
    });
  });

  test("adb's words and its exit code both survive", () => {
    expect(adbFailureText({ exitCode: 1, stderr: "adb.exe: no devices/emulators found" })).toBe(
      "adb.exe: no devices/emulators found (adb exit 1)",
    );
    expect(adbFailureText({ exitCode: 255, message: "spawn adb ENOENT" })).toBe("spawn adb ENOENT (adb exit 255)");
  });

  // The mirror of the defect: `pidof` exits 1 with nothing to say when no
  // process matches, and reading that as a broken transport would fail a wait
  // that was working.
  test("a silent non-zero exit is an answer, not a failure", () => {
    expect(isEmptyDeviceAnswer({ exitCode: 1, stderr: "", stdout: "" })).toBe(true);
    expect(isEmptyDeviceAnswer({ exitCode: 1, stderr: "adb.exe: device 'x' not found" })).toBe(false);
    expect(isEmptyDeviceAnswer({ exitCode: 0, stderr: "", stdout: "" })).toBe(false);
  });
});
