import {
  PowerShellTimeout,
  powershell,
  type PowerShellRunner,
} from "./powershell.js";

export interface ClipboardSnapshot {
  restore(): Promise<void>;
}

export interface ClipboardOptions {
  /** Bound for one PowerShell round trip. Only a test that wants the timeout
   *  diagnostic has a reason to shorten it. */
  timeoutMs?: number;
  /** The harness always uses the real one. The regression for a kill that lands
   *  between the clear and the process exit passes its own, because that window
   *  is too short to aim a timeout at. */
  run?: PowerShellRunner;
}

/**
 * DMY-54, run 31379514744: the job's first PowerShell exceeded 20s and failed
 * the first test file, while the fourteenth file ran the same call three times
 * in 3.7s total. The cost is a cold WinForms assembly load and JIT, not the
 * clipboard API, so the budget has to cover one cold start. It stays finite and
 * is never retried: a hung clipboard owner must still surface as a failure.
 */
const DEFAULT_TIMEOUT_MS = 60_000;

/**
 * Base64 contains no newline, so stdout ending here is a handback the script
 * finished writing — and the clear is ordered after that write. Output that
 * ends with the terminator therefore carries the whole clipboard, whether or
 * not the process lived long enough to clear it.
 */
const HANDBACK_END = /\r?\nEND$/;

const SNAPSHOT_COMMAND = [
  "Add-Type -AssemblyName System.Windows.Forms",
  "$data = [System.Windows.Forms.Clipboard]::GetDataObject()",
  "$formats = if ($null -eq $data) { @() } else { @($data.GetFormats($false)) }",
  "$unicodeText = [System.Windows.Forms.DataFormats]::UnicodeText",
  "$hasText = $null -ne $data -and $data.GetDataPresent($unicodeText, $false)",
  "$isPlainText = $formats.Count -eq 1 -and @($formats)[0] -eq $unicodeText -and $hasText",
  "if ($formats.Count -gt 0 -and -not $isPlainText) { throw " +
    '"the Windows clipboard contains formats that cannot be preserved by ' +
    'this test harness: $($formats -join \', \')" }',
  "$text = if ($hasText) { [string]$data.GetData($unicodeText, $false) } else { $null }",
  "$encoded = if ($null -eq $text) { '' } else { [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($text)) }",
  '[Console]::Out.Write("$encoded`nEND")',
  "[Console]::Out.Flush()",
  "[System.Windows.Forms.Clipboard]::Clear()",
].join("; ");

const SNAPSHOT_WHAT = "the Windows clipboard snapshot";

const NOOP_SNAPSHOT: ClipboardSnapshot = { restore: async () => undefined };

export async function snapshotAndClearClipboard(
  options: ClipboardOptions = {},
): Promise<ClipboardSnapshot> {
  if (process.platform !== "win32") return NOOP_SNAPSHOT;
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const run = options.run ?? powershell;

  let handback: string;
  try {
    handback = await run(SNAPSHOT_COMMAND, SNAPSHOT_WHAT, timeoutMs);
  } catch (error) {
    throw await rescueClearedClipboard(error, timeoutMs, run);
  }

  const encoded = terminatedHandback(handback);
  if (encoded === undefined) {
    throw new Error(
      `${SNAPSHOT_WHAT} exited cleanly without handing the clipboard back, so ` +
        `there is nothing to restore it from.`,
    );
  }

  let restored = false;
  return {
    async restore() {
      if (restored) return;
      await restoreClipboard(encoded, timeoutMs, run);
      restored = true;
    },
  };
}

function terminatedHandback(stdout: string): string | undefined {
  const end = HANDBACK_END.exec(stdout);
  return end ? stdout.slice(0, end.index) : undefined;
}

/**
 * A kill at the budget can land after the script has handed the clipboard back
 * and cleared it. Failing then without putting the content back would cost the
 * user their clipboard for nothing but a slow runner, which is the one outcome
 * this harness must never produce. The run still fails.
 */
async function rescueClearedClipboard(
  error: unknown,
  timeoutMs: number,
  run: PowerShellRunner,
): Promise<unknown> {
  if (!(error instanceof PowerShellTimeout)) return error;
  const encoded = terminatedHandback(error.stdout);
  if (encoded === undefined) return error;
  try {
    await restoreClipboard(encoded, timeoutMs, run);
  } catch (cause) {
    return new Error(
      `${error.message} It had already cleared the clipboard, and putting the ` +
        `content it handed back first was not possible either.`,
      { cause },
    );
  }
  return new Error(
    `${error.message} It had already cleared the clipboard, so the content it ` +
      `handed back first was restored.`,
    { cause: error },
  );
}

async function restoreClipboard(
  encoded: string,
  timeoutMs: number,
  run: PowerShellRunner,
): Promise<void> {
  await run(
    "Add-Type -AssemblyName System.Windows.Forms; " +
      "if ([string]::IsNullOrEmpty($env:COPYPASTE_E2E_CLIPBOARD)) { " +
      "[System.Windows.Forms.Clipboard]::Clear() } else { " +
      "[System.Windows.Forms.Clipboard]::SetText([Text.Encoding]::Unicode.GetString(" +
      "[Convert]::FromBase64String($env:COPYPASTE_E2E_CLIPBOARD))) }",
    "restoring the Windows clipboard",
    timeoutMs,
    { ...process.env, COPYPASTE_E2E_CLIPBOARD: encoded } as Record<string, string>,
  );
}
