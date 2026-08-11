import { appendFileSync, writeFileSync } from "node:fs";

import { NATIVE_DRIVER, RUN_ROOT, runLogPath } from "./env.js";
import { powershell } from "./powershell.js";

export interface RunManifestOptions {
  path?: string;
  /** The harness probes WinForms on Windows and nothing anywhere else. A test
   *  passes its own to reproduce a probe that fails. */
  probe?: () => Promise<unknown>;
}

const PROBE_TIMEOUT_MS = 60_000;

/**
 * Written before anything else can fail, including the probe below: run
 * 31379514744 uploaded an empty artifact because the only file under the log
 * root was written after the step that died. The probe itself pays the cold
 * WinForms load that DMY-54's first test file used to pay inside its own
 * budget, and records what it cost. It reads no clipboard content and writes
 * none.
 */
export async function recordRunEnvironment(
  options: RunManifestOptions = {},
): Promise<void> {
  const manifest = options.path ?? runLogPath("run.log");
  writeFileSync(
    manifest,
    `platform=${process.platform} node=${process.version}\n` +
      `runRoot=${RUN_ROOT}\n` +
      `nativeDriver=${NATIVE_DRIVER}\n`,
  );

  const probe =
    options.probe ?? (process.platform === "win32" ? loadWinForms : undefined);
  if (!probe) return;

  const started = Date.now();
  try {
    await probe();
  } catch (cause) {
    const elapsed = Date.now() - started;
    appendFileSync(
      manifest,
      `powershellWinFormsColdStartFailedMs=${elapsed}\n` +
        `powershellWinFormsColdStartError=${describe(cause)}\n`,
    );
    throw new Error(
      `PowerShell could not load System.Windows.Forms in ${elapsed}ms, so every ` +
        `clipboard call this suite makes would fail the same way. The run log ` +
        `records the failure; fix the host before reading any later timeout as ` +
        `a product fault.`,
      { cause },
    );
  }
  appendFileSync(
    manifest,
    `powershellWinFormsColdStartMs=${Date.now() - started}\n`,
  );
}

async function loadWinForms(): Promise<unknown> {
  return powershell(
    "Add-Type -AssemblyName System.Windows.Forms",
    "the PowerShell WinForms load",
    PROBE_TIMEOUT_MS,
  );
}

function describe(cause: unknown): string {
  const message = cause instanceof Error ? cause.message : String(cause);
  return message.replace(/\s+/g, " ").trim();
}
