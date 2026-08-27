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
  webview2Version?: string;
}

const VERSION_TIMEOUT_MS = 15_000;

const EDGE_VERSION_SCRIPT = String.raw`
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
Write-Output $edgeVersions[0]
`;

export function parseVersion(output: string, what: string): string {
  const match = output.match(/\b(\d+\.\d+\.\d+\.\d+)\b/);
  if (!match) throw new Error(`${what} did not report a four-part version`);
  return match[1]!;
}

export function assertMajorCompatibility(
  versions: WindowsEnvironmentVersions,
): void {
  const builds = Object.fromEntries(
    Object.entries(versions).map(([name, version]) => [name, firstThree(version)]),
  );
  const distinct = new Set(Object.values(builds));
  if (distinct.size !== 1) {
    throw new Error(
      `Edge/WebView2/EdgeDriver first-three-part versions are incompatible ` +
        `(Edge ${builds.edge}, WebView2 ${builds.webview2}, ` +
        `EdgeDriver ${builds.edgeDriver})`,
    );
  }
}

export async function probeWindowsEnvironment(
  options: WindowsEnvironmentProbeOptions,
): Promise<WindowsEnvironmentVersions> {
  const runPowerShell = options.powershell ?? powershell;
  const driver = options.driver ?? NATIVE_DRIVER;
  const webview2Version =
    options.webview2Version ?? process.env.COPYPASTE_WEBVIEW2_RUNTIME_VERSION;
  try {
    if (!webview2Version) {
      throw new Error("COPYPASTE_WEBVIEW2_RUNTIME_VERSION is unset");
    }
    const [edgeOutput, driverResult] = await Promise.all([
      runPowerShell(
        EDGE_VERSION_SCRIPT,
        "the Microsoft Edge version probe",
        VERSION_TIMEOUT_MS,
      ),
      options.driverVersion
        ? options.driverVersion()
        : execa(driver, ["--version"], { timeout: VERSION_TIMEOUT_MS }).then(
            (result) => result.stdout,
          ),
    ]);
    const versions = {
      edge: parseVersion(edgeOutput, "Microsoft Edge"),
      webview2: parseVersion(webview2Version, "WebView2 Runtime"),
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
        "Edge, WebView2 Runtime, and EdgeDriver must be installed and first-three-part-compatible.",
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
    appendSessionFailure(manifest, "startup", cause);
    throw new WindowsEnvironmentProbeFailure(
      "Windows environment probe failed: Tauri could not establish a ready WebView2 session; " +
        "the native suite was not started.",
      { cause },
    );
  } finally {
    if (session) {
      try {
        await session.stop();
      } catch (cause) {
        appendSessionFailure(manifest, "cleanup", cause);
        throw new WindowsEnvironmentProbeFailure(
          "Windows environment probe failed during Tauri session cleanup; " +
            "the native suite was not started.",
          { cause },
        );
      }
    }
  }
}

function appendSessionFailure(
  manifest: string,
  phase: "startup" | "cleanup",
  cause: unknown,
): void {
  appendFileSync(
    manifest,
    `tauriSessionProbe=failed\n` +
      `tauriSessionProbeFailure=${phase}\n` +
      `tauriSessionProbe${phase === "cleanup" ? "Cleanup" : ""}Error=${describe(cause)}\n`,
  );
}

function firstThree(version: string): string {
  const parts = version.split(".");
  if (parts.length !== 4 || parts.some((part) => !/^\d+$/.test(part))) {
    throw new Error(`invalid version ${version}`);
  }
  return parts.slice(0, 3).join(".");
}

function describe(cause: unknown): string {
  const message = cause instanceof Error ? cause.message : String(cause);
  return message.replace(/\s+/g, " ").trim();
}
