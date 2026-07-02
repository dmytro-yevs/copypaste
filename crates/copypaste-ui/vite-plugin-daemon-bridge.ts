// DEV-ONLY bridge: exposes the CopyPaste daemon Unix socket to the Vite dev
// server at POST /__ipc so a browser-driven UI (Playwright, ?bridge=1) talks to
// the REAL daemon with REAL data. `apply: "serve"` guarantees it is NEVER part
// of a production `vite build` / `tauri build`. Do not remove that guard.
import type { Plugin } from "vite";
import net from "node:net";
import os from "node:os";
import path from "node:path";

const SOCKET_TIMEOUT_MS = 10_000;

function socketPath(): string {
  if (process.env.COPYPASTE_SOCKET) return process.env.COPYPASTE_SOCKET;
  // Mirror copypaste-daemon::paths::socket_path (macOS).
  return path.join(
    os.homedir(),
    "Library/Application Support/CopyPaste/daemon.sock",
  );
}

function callDaemon(method: string, params: unknown): Promise<unknown> {
  return new Promise((resolve) => {
    const sock = net.connect({ path: socketPath() });
    let buf = "";
    const finish = (v: unknown) => {
      sock.destroy();
      resolve(v);
    };
    const timer = setTimeout(
      () => finish({ ok: false, error_code: "io_error", error: "timeout" }),
      SOCKET_TIMEOUT_MS,
    );
    sock.on("connect", () => {
      sock.write(
        JSON.stringify({
          id: "ui-1",
          method,
          params: params ?? null,
          protocol_version: 1,
        }) + "\n",
      );
    });
    sock.on("data", (d) => {
      buf += d.toString("utf8");
      const nl = buf.indexOf("\n");
      if (nl === -1) return;
      clearTimeout(timer);
      try {
        finish(JSON.parse(buf.slice(0, nl)));
      } catch {
        finish({ ok: false, error_code: "invalid_json" });
      }
    });
    sock.on("error", (e: NodeJS.ErrnoException) => {
      clearTimeout(timer);
      const offline = e.code === "ENOENT" || e.code === "ECONNREFUSED";
      finish({
        ok: false,
        error_code: offline ? "daemon_offline" : "io_error",
        error: String(e.message ?? e),
      });
    });
  });
}

export function daemonBridge(): Plugin {
  return {
    name: "copypaste-daemon-bridge",
    apply: "serve", // DEV server only — never in `vite build`.
    configureServer(server) {
      server.middlewares.use("/__ipc", (req, res) => {
        if (req.method !== "POST") {
          res.statusCode = 405;
          res.end();
          return;
        }
        let body = "";
        req.on("data", (c) => (body += c));
        req.on("end", async () => {
          let parsed: { method?: string; params?: unknown };
          try {
            parsed = JSON.parse(body || "{}");
          } catch {
            res.statusCode = 400;
            res.end(JSON.stringify({ ok: false, error: "bad_json" }));
            return;
          }
          if (!parsed.method) {
            res.statusCode = 400;
            res.end(JSON.stringify({ ok: false, error: "missing_method" }));
            return;
          }
          const reply = await callDaemon(parsed.method, parsed.params);
          res.setHeader("content-type", "application/json");
          res.end(JSON.stringify(reply));
        });
      });
    },
  };
}
