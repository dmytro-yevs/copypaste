import { execa } from "execa";
import { afterAll, afterEach, beforeAll, describe, expect, test } from "vitest";

import {
  snapshotAndClearClipboard,
  type ClipboardSnapshot,
} from "../src/harness/clipboard.js";

let originalClipboard: ClipboardSnapshot | undefined;

async function runClipboardScript(script: string): Promise<string> {
  const result = await execa(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      `Add-Type -AssemblyName System.Windows.Forms; ${script}`,
    ],
    { timeout: 20_000 },
  );
  return result.stdout;
}

beforeAll(async () => {
  if (process.platform !== "win32") {
    throw new Error("the Windows clipboard harness suite ran on a non-Windows host");
  }
  originalClipboard = await snapshotAndClearClipboard();
});

afterEach(async () => {
  await runClipboardScript("[System.Windows.Forms.Clipboard]::Clear()");
});

afterAll(async () => {
  await originalClipboard?.restore();
});

describe("Windows clipboard snapshot isolation", () => {
  test("accepts and restores an empty clipboard", async () => {
    const snapshot = await snapshotAndClearClipboard();

    await snapshot.restore();

    expect(
      await runClipboardScript(
        "$data = [System.Windows.Forms.Clipboard]::GetDataObject(); " +
          "$formats = if ($null -eq $data) { @() } else { @($data.GetFormats($false)) }; " +
          "[Console]::Out.Write($formats.Count)",
      ),
    ).toBe("0");
  });

  test("accepts, clears, and restores ordinary Unicode text", async () => {
    await runClipboardScript(
      "[System.Windows.Forms.Clipboard]::SetText('plain clipboard fixture')",
    );

    const snapshot = await snapshotAndClearClipboard();
    expect(
      await runClipboardScript(
        "$data = [System.Windows.Forms.Clipboard]::GetDataObject(); " +
          "$formats = if ($null -eq $data) { @() } else { @($data.GetFormats($false)) }; " +
          "[Console]::Out.Write($formats.Count)",
      ),
    ).toBe("0");

    await snapshot.restore();

    expect(
      await runClipboardScript(
        "[Console]::Out.Write([System.Windows.Forms.Clipboard]::GetText())",
      ),
    ).toBe("plain clipboard fixture");
  });

  test("rejects mixed Unicode text and HTML without clearing", async () => {
    await runClipboardScript(
      "$data = [System.Windows.Forms.DataObject]::new(); " +
        "$data.SetData([System.Windows.Forms.DataFormats]::UnicodeText, $false, 'mixed text fixture'); " +
        "$data.SetData([System.Windows.Forms.DataFormats]::Html, $false, '<b>mixed html fixture</b>'); " +
        "[System.Windows.Forms.Clipboard]::SetDataObject($data, $true)",
    );

    await expect(snapshotAndClearClipboard()).rejects.toThrow(
      /formats that cannot be preserved/,
    );

    expect(
      await runClipboardScript(
        "$data = [System.Windows.Forms.Clipboard]::GetDataObject(); " +
          "$text = $data.GetData([System.Windows.Forms.DataFormats]::UnicodeText, $false); " +
          "$html = $data.GetData([System.Windows.Forms.DataFormats]::Html, $false); " +
          "[Console]::Out.Write(('{0}|{1}' -f $text, $html))",
      ),
    ).toBe("mixed text fixture|<b>mixed html fixture</b>");
  });
});
