import { ExecaError, execa } from "execa";

export type PowerShellRunner = (
  command: string,
  what: string,
  timeoutMs: number,
  env?: Record<string, string>,
) => Promise<string>;

/**
 * Carries what the killed process had already printed. A caller whose script
 * destroys something after it prints needs that output to undo the damage; the
 * message alone is not enough.
 */
export class PowerShellTimeout extends Error {
  readonly stdout: string;

  constructor(message: string, stdout: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "PowerShellTimeout";
    this.stdout = stdout;
  }
}

/** One bounded PowerShell round trip. Never retried: a hung host process must
 *  still surface as a failure rather than as a second attempt. */
export const powershell: PowerShellRunner = async (
  command,
  what,
  timeoutMs,
  env,
) => {
  const started = Date.now();
  try {
    const result = await execa(
      "powershell.exe",
      ["-NoProfile", "-NonInteractive", "-Command", command],
      env ? { timeout: timeoutMs, env } : { timeout: timeoutMs },
    );
    return result.stdout;
  } catch (cause) {
    if (cause instanceof ExecaError && cause.timedOut) {
      throw new PowerShellTimeout(
        `${what} did not finish within ${timeoutMs}ms (${Date.now() - started}ms ` +
          `elapsed) and PowerShell was killed. Compare that against the budget ` +
          `before reading this as a hang: the first call of a run is far slower ` +
          `than every later one.`,
        typeof cause.stdout === "string" ? cause.stdout : "",
        { cause },
      );
    }
    throw cause;
  }
};
