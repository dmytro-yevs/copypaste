import { PACKAGE, appPid, forward, removeForward, shell, sleep } from "./adb.js";

export const DEFAULT_PORT = Number(process.env.COPYPASTE_DEVTOOLS_PORT ?? 9222);

export interface DevtoolsEndpoint {
  browserUrl: string;
  port: number;
  pid: number;
  socket: string;
  version: Record<string, string>;
}

/**
 * The WebView's abstract socket carries the app's **pid** in its name, so it is
 * a different socket after every restart.
 *
 * This matters more than it looks. `adb forward` binds the local port whether
 * or not the remote name still exists, and a connection to a stale forward is
 * accepted and then closed with no bytes — `curl` calls that "Empty reply from
 * server" and undici calls it "other side closed". Neither says "the app
 * restarted", which is the only thing it ever means. Resolve the name from the
 * device immediately before forwarding, and re-resolve on any failure to read
 * `/json/version` rather than reporting the endpoint as broken.
 */
const SOCKET_LINE = /@(webview_devtools_remote_(\d+))$/;

async function devtoolsSockets(): Promise<Map<number, string>> {
  const found = new Map<number, string>();
  for (const line of (await shell("cat", "/proc/net/unix")).split("\n")) {
    const match = SOCKET_LINE.exec(line.trim());
    if (match) found.set(Number(match[2]), match[1]);
  }
  return found;
}

async function version(port: number): Promise<Record<string, string> | undefined> {
  try {
    const response = await fetch(`http://127.0.0.1:${port}/json/version`, {
      signal: AbortSignal.timeout(5_000),
    });
    if (!response.ok) return undefined;
    return (await response.json()) as Record<string, string>;
  } catch {
    return undefined;
  }
}

/**
 * Forward the running app's WebView devtools socket and return an endpoint that
 * has been proved to answer.
 */
export async function openDevtools(
  port = DEFAULT_PORT,
  timeoutMs = 90_000,
): Promise<DevtoolsEndpoint> {
  const deadline = Date.now() + timeoutMs;
  let why = "no attempt was made";

  while (Date.now() < deadline) {
    const pid = await appPid();
    if (pid === undefined) {
      why = `no process named ${PACKAGE} is running`;
      await sleep(1_000);
      continue;
    }

    const socket = (await devtoolsSockets()).get(pid);
    if (socket === undefined) {
      why = `pid ${pid} is running but has published no webview_devtools_remote socket`;
      await sleep(1_000);
      continue;
    }

    await forward(port, socket);
    const answered = await version(port);
    if (answered === undefined) {
      why = `${socket} did not answer /json/version`;
      await sleep(1_000);
      continue;
    }

    // The WebView zygote publishes sockets too. This is the field that says the
    // renderer behind this one belongs to the app under test.
    const owner = answered["Android-Package"];
    if (owner !== PACKAGE) {
      throw new Error(
        `the devtools endpoint on ${socket} belongs to ${owner ?? "an unnamed package"}, not ${PACKAGE}`,
      );
    }

    return { browserUrl: `http://127.0.0.1:${port}`, port, pid, socket, version: answered };
  }

  throw new Error(
    `no WebView devtools endpoint after ${Math.round(timeoutMs / 1000)}s: ${why}. ` +
      `A debug build enables it; a release build compiles the call away (see e2e-android/README.md).`,
  );
}

export async function closeDevtools(port = DEFAULT_PORT): Promise<void> {
  await removeForward(port);
}
