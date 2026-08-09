import { execa } from "execa";

export interface ClipboardSnapshot {
  restore(): Promise<void>;
}

const NOOP_SNAPSHOT: ClipboardSnapshot = { restore: async () => undefined };

export async function snapshotAndClearClipboard(): Promise<ClipboardSnapshot> {
  if (process.platform !== "win32") return NOOP_SNAPSHOT;

  const result = await execa(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      [
        "Add-Type -AssemblyName System.Windows.Forms",
        "$data = [System.Windows.Forms.Clipboard]::GetDataObject()",
        "$formats = if ($null -eq $data) { @() } else { @($data.GetFormats($false)) }",
        "$unicodeText = [System.Windows.Forms.DataFormats]::UnicodeText",
        "$hasText = $null -ne $data -and $data.GetDataPresent($unicodeText, $false)",
        "$isPlainText = $formats.Count -eq 1 -and @($formats)[0] -eq $unicodeText -and $hasText",
        "if ($formats.Count -gt 0 -and -not $isPlainText) { throw 'the Windows clipboard contains formats that cannot be preserved by this test harness' }",
        "$text = if ($hasText) { [string]$data.GetData($unicodeText, $false) } else { $null }",
        "$encoded = if ($null -eq $text) { '' } else { [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($text)) }",
        "[System.Windows.Forms.Clipboard]::Clear()",
        "[Console]::Out.Write($encoded)",
      ].join("; "),
    ],
    { timeout: 20_000 },
  );

  const encoded = result.stdout;
  let restored = false;
  return {
    async restore() {
      if (restored) return;
      await execa(
        "powershell.exe",
        [
          "-NoProfile",
          "-NonInteractive",
          "-Command",
          "Add-Type -AssemblyName System.Windows.Forms; " +
            "if ([string]::IsNullOrEmpty($env:COPYPASTE_E2E_CLIPBOARD)) { " +
            "[System.Windows.Forms.Clipboard]::Clear() } else { " +
            "[System.Windows.Forms.Clipboard]::SetText([Text.Encoding]::Unicode.GetString(" +
            "[Convert]::FromBase64String($env:COPYPASTE_E2E_CLIPBOARD))) }",
        ],
        {
          env: { ...process.env, COPYPASTE_E2E_CLIPBOARD: encoded },
          timeout: 20_000,
        },
      );
      restored = true;
    },
  };
}
