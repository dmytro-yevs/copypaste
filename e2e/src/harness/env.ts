import { existsSync } from "node:fs";
import { createServer } from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const E2E_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
export const REPO_ROOT = path.resolve(E2E_ROOT, "..");
export const UI_DIR = path.join(REPO_ROOT, "crates/copypaste-ui");

/**
 * The app is a Tauri *dev* build unless it was built for release: a debug
 * binary loads `devUrl` (`http://localhost:1420`) rather than the bundled
 * `frontendDist`. `strictPort` is set in vite.config.ts, so exactly one dev
 * server can exist per run — hence global setup owns it and test files do not.
 */
export const DEV_SERVER_PORT = 1420;
export const DEV_SERVER_URL = `http://localhost:${DEV_SERVER_PORT}`;

function fromEnvOrTarget(envVar: string, name: string): string {
  const override = process.env[envVar];
  if (override) return override;
  for (const profile of ["debug", "release"]) {
    const candidate = path.join(REPO_ROOT, "target", profile, name);
    if (existsSync(candidate)) return candidate;
  }
  throw new Error(
    `${name} not found in target/debug or target/release. Build it ` +
      `(\`cargo build -p ${name === "copypaste" ? "copypaste-cli" : name}\`) ` +
      `or set ${envVar} to its path.`,
  );
}

export const appBinary = () => fromEnvOrTarget("COPYPASTE_UI_BIN", "copypaste-ui");
export const daemonBinary = () =>
  fromEnvOrTarget("COPYPASTE_DAEMON_BIN", "copypaste-daemon");
export const cliBinary = () => fromEnvOrTarget("COPYPASTE_CLI_BIN", "copypaste");

export const NATIVE_DRIVER = process.env.WEBKIT_WEBDRIVER ?? "/usr/bin/WebKitWebDriver";

/**
 * A Unix socket path must fit in `sockaddr_un.sun_path` (~108 bytes) or the
 * daemon dies with "path must be shorter than SUN_LEN". The daemon derives its
 * socket from `XDG_DATA_HOME`, and the usual per-run scratch directories are
 * already long enough to blow the limit, so run roots live directly under /tmp.
 */
export const RUN_ROOT = "/tmp/cp-e2e";

export async function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.unref();
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (address === null || typeof address === "string") {
        server.close();
        reject(new Error("could not allocate a port"));
        return;
      }
      const { port } = address;
      server.close(() => resolve(port));
    });
  });
}

export function requireDisplay(): string {
  const display = process.env.DISPLAY;
  if (!display) {
    throw new Error(
      "DISPLAY is unset. These tests drive a real WebKitGTK window; run them " +
        "through `npm test`, which wraps vitest in xvfb-run.",
    );
  }
  return display;
}
