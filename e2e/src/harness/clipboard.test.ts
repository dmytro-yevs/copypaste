import { execFile } from "node:child_process";
import { promisify } from "node:util";

import { describe, expect, test } from "vitest";

import {
  snapshotAndClearClipboard,
  WINDOWS_CLIPBOARD_FORMAT_GUARD,
} from "./clipboard.js";
import type { PowerShellRunner } from "./powershell.js";

const runFile = promisify(execFile);
const UNICODE_HANDOFF = "UwBtAGkAbABlACAAPdgA3g==";

describe("Windows clipboard snapshot format policy", () => {
  test.each([
    ["an empty clipboard", [], false, true],
    ["UnicodeText", ["UnicodeText"], true, true],
    [
      "Windows synthesized Unicode text aliases",
      ["UnicodeText", "Text", "OEMText", "Locale"],
      true,
      true,
    ],
    [
      "text aliases without usable UnicodeText",
      ["Text", "OEMText", "Locale"],
      false,
      false,
    ],
    [
      "reported data without a UnicodeText format",
      ["Text", "OEMText", "Locale"],
      true,
      false,
    ],
    [
      "a listed but unavailable UnicodeText value",
      ["UnicodeText", "Text"],
      false,
      false,
    ],
    ["HTML", ["UnicodeText", "HTML Format"], true, false],
    ["RTF", ["UnicodeText", "Rich Text Format"], true, false],
    ["a bitmap", ["UnicodeText", "Bitmap"], true, false],
    ["file drops", ["UnicodeText", "FileDrop"], true, false],
    [
      "a custom format",
      ["UnicodeText", "Example.Application.Payload"],
      true,
      false,
    ],
  ] as const)(
    "accepts only %s",
    async (_, formats, hasUnicodeText, expected) => {
      expect(await evaluateFormatGuard(formats, hasUnicodeText)).toBe(expected);
    },
  );

  test("keeps the format rule aligned with snapshot handback and one restore", async () => {
    const calls: Parameters<PowerShellRunner>[] = [];
    const run: PowerShellRunner = async (...args) => {
      calls.push(args);
      return args[1] === "the Windows clipboard snapshot"
        ? `${UNICODE_HANDOFF}\nEND`
        : "";
    };

    await onWindows(async () => {
      const snapshot = await snapshotAndClearClipboard({ run });
      await snapshot.restore();
      await snapshot.restore();
    });

    const [command, what] = calls[0]!;
    expect(what).toBe("the Windows clipboard snapshot");
    expect(command).toContain(WINDOWS_CLIPBOARD_FORMAT_GUARD);
    expect(command).toContain(
      "$hasUnicodeText = $null -ne $data -and $data.GetDataPresent($unicodeText, $false)",
    );
    expect(calls).toHaveLength(2);
    expect(calls[1]?.[3]?.COPYPASTE_E2E_CLIPBOARD).toBe(UNICODE_HANDOFF);
    expect(Buffer.from(UNICODE_HANDOFF, "base64").toString("utf16le")).toBe(
      "Smile 😀",
    );
  });
});

async function evaluateFormatGuard(
  formats: readonly string[],
  hasUnicodeText: boolean,
): Promise<boolean> {
  const script = [
    `$formats = @(${formats.map((format) => `'${format}'`).join(", ")})`,
    `$hasUnicodeText = $${hasUnicodeText}`,
    "try {",
    WINDOWS_CLIPBOARD_FORMAT_GUARD,
    "[Console]::Out.Write('accepted')",
    "} catch {",
    "[Console]::Out.Write('rejected')",
    "}",
  ].join("; ");
  const { stdout } = await runFile("pwsh", ["-NoProfile", "-Command", script]);
  return stdout === "accepted";
}

async function onWindows(action: () => Promise<void>): Promise<void> {
  const descriptor = Object.getOwnPropertyDescriptor(process, "platform");
  Object.defineProperty(process, "platform", {
    configurable: true,
    value: "win32",
  });
  try {
    await action();
  } finally {
    Object.defineProperty(process, "platform", descriptor!);
  }
}
