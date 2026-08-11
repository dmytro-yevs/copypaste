import { execa } from "execa";
import { afterAll, afterEach, beforeAll, describe, expect, test } from "vitest";

import {
  snapshotAndClearClipboard,
  type ClipboardSnapshot,
} from "../src/harness/clipboard.js";
import {
  PowerShellTimeout,
  powershell,
  type PowerShellRunner,
} from "../src/harness/powershell.js";

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

    const refusal = await snapshotAndClearClipboard().then(
      () => undefined,
      (error: Error) => error,
    );
    expect(refusal?.message).toMatch(/formats that cannot be preserved/);
    // Naming them is the difference between a developer knowing which tool owns
    // the clipboard and rerunning the suite to find out.
    expect(refusal?.message).toContain("UnicodeText");
    expect(refusal?.message).toContain("HTML Format");

    expect(
      await runClipboardScript(
        "$data = [System.Windows.Forms.Clipboard]::GetDataObject(); " +
          "$text = $data.GetData([System.Windows.Forms.DataFormats]::UnicodeText, $false); " +
          "$html = $data.GetData([System.Windows.Forms.DataFormats]::Html, $false); " +
          "[Console]::Out.Write(('{0}|{1}' -f $text, $html))",
      ),
    ).toBe("mixed text fixture|<b>mixed html fixture</b>");
  });

  /**
   * DMY-54: a 20s bound was exhausted by the job's first PowerShell and the
   * harness reported only execa's own "Command timed out", which reads as a
   * hung clipboard. The budget has to be in the message, and exhausting it must
   * not cost the user their clipboard.
   */
  test("reports an exhausted budget and leaves the clipboard alone", async () => {
    await runClipboardScript(
      "[System.Windows.Forms.Clipboard]::SetText('budget fixture')",
    );

    const exhausted = await snapshotAndClearClipboard({ timeoutMs: 1 }).then(
      () => undefined,
      (error: Error) => error,
    );
    expect(exhausted?.message).toMatch(
      /the Windows clipboard snapshot did not finish within 1ms/,
    );
    expect(exhausted?.message).toMatch(/\d+ms elapsed/);

    expect(
      await runClipboardScript(
        "[Console]::Out.Write([System.Windows.Forms.Clipboard]::GetText())",
      ),
    ).toBe("budget fixture");
  });

  /**
   * The window the previous test cannot reach: the kill lands after the script
   * has handed the clipboard back and cleared it. The real script runs and
   * really clears; only the moment of the kill is supplied, because a timeout
   * cannot be aimed at a window this short.
   */
  test("restores the clipboard when the kill lands after the clear", async () => {
    await runClipboardScript(
      "[System.Windows.Forms.Clipboard]::SetText('post-clear fixture')",
    );

    const killedAfterTheClear: PowerShellRunner = async (
      command,
      what,
      timeoutMs,
      env,
    ) => {
      if (what !== "the Windows clipboard snapshot") {
        return powershell(command, what, timeoutMs, env);
      }
      const stdout = await powershell(command, what, 60_000);
      throw new PowerShellTimeout(
        `${what} did not finish within ${timeoutMs}ms (${timeoutMs}ms elapsed) ` +
          `and PowerShell was killed.`,
        stdout,
      );
    };

    const killed = await snapshotAndClearClipboard({
      timeoutMs: 60_000,
      run: killedAfterTheClear,
    }).then(
      () => undefined,
      (error: Error) => error,
    );
    expect(
      await runClipboardScript(
        "[Console]::Out.Write([System.Windows.Forms.Clipboard]::GetText())",
      ),
    ).toBe("post-clear fixture");
    expect(killed?.message).toMatch(/did not finish within 60000ms/);
    expect(killed?.message).toMatch(/handed back first was restored/);
  });
});
