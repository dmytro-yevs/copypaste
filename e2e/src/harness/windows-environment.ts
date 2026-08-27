import { appendFileSync } from "node:fs";

import { execa } from "execa";

import { NATIVE_DRIVER } from "./env.js";
import { powershell } from "./powershell.js";

export interface WindowsEnvironmentVersions {
  edge: string;
  webview2: string;
  edgeDriver: string;
}

export class WindowsEnvironmentProbeFailure extends Error {
  readonly kind = "environment";

  constructor(message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "WindowsEnvironmentProbeFailure";
  }
}

export interface WindowsEnvironmentProbeOptions {
  manifest: string;
  powershell?: typeof powershell;
  driver?: string;
  driverVersion?: () => Promise<string>;
}

const VERSION_TIMEOUT_MS = 15_000;

const VERSION_SCRIPT = String.raw`
$product = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
$keys = @(
  "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$product"
  "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$product"
)
$versions = @($keys | ForEach-Object {
  (Get-ItemProperty -LiteralPath $_ -Name pv -ErrorAction SilentlyContinue).pv
} | Where-Object { $_ -and $_ -ne "0.0.0.0" } | Sort-Object -Unique)
if ($versions.Count -ne 1) { throw "expected one installed WebView2 Runtime version" }
$runtime = [string]$versions[0]

$programFilesX86 = [Environment]::GetEnvironmentVariable("ProgramFiles(x86)")
$edgePaths = @(
  (Join-Path $env:ProgramFiles "Microsoft\Edge\Application\msedge.exe")
  (Join-Path $programFilesX86 "Microsoft\Edge\Application\msedge.exe")
  (Join-Path $env:LOCALAPPDATA "Microsoft\Edge\Application\msedge.exe")
) | Where-Object { $_ -and (Test-Path -LiteralPath $_) }
$edgeVersions = @($edgePaths | ForEach-Object {
  (Get-Item -LiteralPath $_).VersionInfo.ProductVersion
} | Where-Object { $_ } | Sort-Object -Unique)
if ($edgeVersions.Count -ne 1) { throw "expected one installed Microsoft Edge version" }

Write-Output "$($edgeVersions[0])|$runtime"
`;

export function parseVersion(output: string, what: string): string {
  const match = output.match(/\b(\d+\.\d+\.\d+\.\d+)\b/);
  if (!match) throw new Error(`${what} did not report a four-part version`);
  return match[1]!;
}

export function assertMajorCompatibility(
  versions: WindowsEnvironmentVersions,
): void {
  const majors = Object.fromEntries(
    Object.entries(versions).map(([name, version]) => [name, major(version)]),
  );
  const distinct = new Set(Object.values(majors));
  if (distinct.size !== 1) {
    throw new Error(
      `Edge/WebView2/EdgeDriver major versions are incompatible ` +
        `(Edge ${majors.edge}, WebView2 ${majors.webview2}, ` +
        `EdgeDriver ${majors.edgeDriver})`,
    );
  }
}

export async function probeWindowsEnvironment(
  options: WindowsEnvironmentProbeOptions,
): Promise<WindowsEnvironmentVersions> {
  const runPowerShell = options.powershell ?? powershell;
  const driver = options.driver ?? NATIVE_DRIVER;
  try {
    const [edgeAndRuntime, driverResult] = await Promise.all([
      runPowerShell(
        VERSION_SCRIPT,
        "the Edge/WebView2 version probe",
        VERSION_TIMEOUT_MS,
      ),
      options.driverVersion
        ? options.driverVersion()
        : execa(driver, ["--version"], { timeout: VERSION_TIMEOUT_MS }).then(
            (result) => result.stdout,
          ),
    ]);
    const [edgeOutput, webview2Output] = edgeAndRuntime.trim().split("|");
    if (!edgeOutput || !webview2Output) {
      throw new Error("the Edge/WebView2 version probe returned incomplete output");
    }
    const versions = {
      edge: parseVersion(edgeOutput, "Microsoft Edge"),
      webview2: parseVersion(webview2Output, "WebView2 Runtime"),
      edgeDriver: parseVersion(driverResult, "Edge WebDriver"),
    };
    assertMajorCompatibility(versions);
    appendFileSync(
      options.manifest,
      `edgeVersion=${versions.edge}\n` +
        `webview2RuntimeVersion=${versions.webview2}\n` +
        `edgeDriverVersion=${versions.edgeDriver}\n` +
        `windowsEnvironmentProbe=ready\n`,
    );
    return versions;
  } catch (cause) {
    appendFileSync(
      options.manifest,
      `windowsEnvironmentProbe=failed\n` +
        `windowsEnvironmentProbeError=${describe(cause)}\n`,
    );
    throw new WindowsEnvironmentProbeFailure(
      "Windows environment probe failed before the native E2E suite; " +
        "Edge, WebView2 Runtime, and EdgeDriver must be installed and major-compatible.",
      { cause },
    );
  }
}

export async function probeTauriSession<T extends { stop(): Promise<void> }>(
  start: () => Promise<T>,
  manifest: string,
): Promise<void> {
  let session: T | undefined;
  try {
    session = await start();
    appendFileSync(manifest, "tauriSessionProbe=ready\n");
  } catch (cause) {
    appendFileSync(
      manifest,
      `tauriSessionProbe=failed\n` +
        `tauriSessionProbeError=${describe(cause)}\n`,
    );
    throw new WindowsEnvironmentProbeFailure(
      "Windows environment probe failed: Tauri could not establish a ready WebView2 session; " +
        "the native suite was not started.",
      { cause },
    );
  } finally {
    await session?.stop();
  }
}

function major(version: string): number {
  const value = Number.parseInt(version.split(".")[0] ?? "", 10);
  if (!Number.isSafeInteger(value)) throw new Error(`invalid version ${version}`);
  return value;
}

function describe(cause: unknown): string {
  const message = cause instanceof Error ? cause.message : String(cause);
  return message.replace(/\s+/g, " ").trim();
}
