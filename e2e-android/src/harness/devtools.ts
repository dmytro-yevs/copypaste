import { PACKAGE, appPid, forward, removeForward, tryShell } from "./adb.js";
import { classifyAdbFailure } from "./adb-failure.js";
import { waitForReadiness, type Probe } from "./readiness.js";

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

export function socketsInProcNetUnix(dump: string): Map<number, string> {
  const found = new Map<number, string>();
  for (const line of dump.split("\n")) {
    const match = SOCKET_LINE.exec(line.trim());
    if (match) found.set(Number(match[2]), match[1]!);
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
 * The worst case for one probe: `ps`, then `pidof` when the process list did not
 * name the app, `cat /proc/net/unix`, and `adb forward --remove` with the
 * `adb forward` behind it. Declaring the worst case is what makes the budget a
 * ceiling rather than an estimate.
 */
const PROCESSES_PER_PROBE = 5;

/**
 * Bounded in adb processes, not in seconds. The fixed 1s poll this replaced ran
 * one probe per second for the whole timeout — 90 probes and up to 450 adb
 * launches on a 90s wait, 150 and 750 on the 150s one `app.ts` asks for. The
 * backoff below settles at 5s, so the same 150s costs 29 probes; the budget caps
 * it whatever the caller passes.
 */
const PROCESS_BUDGET = Number(process.env.COPYPASTE_DEVTOOLS_PROCESS_BUDGET ?? 160);

async function probeDevtools(port: number): Promise<Probe<DevtoolsEndpoint>> {
  const pid = await appPid();
  if (pid === undefined) return { kind: "not-ready", why: `no process named ${PACKAGE} is running` };

  const dump = await tryShell("cat", "/proc/net/unix");
  if (!dump.ok) return classifyAdbFailure(dump.failure);

  const socket = socketsInProcNetUnix(dump.value).get(pid);
  if (socket === undefined) {
    return {
      kind: "not-ready",
      why: `pid ${pid} is running but has published no webview_devtools_remote socket`,
    };
  }

  await forward(port, socket);
  const answered = await version(port);
  if (answered === undefined) return { kind: "not-ready", why: `${socket} did not answer /json/version` };

  // The WebView zygote publishes sockets too. This is the field that says the
  // renderer behind this one belongs to the app under test. Not a wait: another
  // package's endpoint does not become ours by being asked again.
  const owner = answered["Android-Package"];
  if (owner !== PACKAGE) {
    return {
      kind: "invariant",
      why: `the devtools endpoint on ${socket} belongs to ${owner ?? "an unnamed package"}, not ${PACKAGE}`,
    };
  }

  return {
    kind: "ready",
    value: { browserUrl: `http://127.0.0.1:${port}`, port, pid, socket, version: answered },
  };
}

/**
 * Forward the running app's WebView devtools socket and return an endpoint that
 * has been proved to answer.
 */
export async function openDevtools(
  port = DEFAULT_PORT,
  timeoutMs = 90_000,
): Promise<DevtoolsEndpoint> {
  return waitForReadiness<DevtoolsEndpoint>({
    description: `a WebView devtools endpoint for ${PACKAGE}`,
    timeoutMs,
    processBudget: PROCESS_BUDGET,
    processesPerProbe: PROCESSES_PER_PROBE,
    maxDelayMs: 5_000,
    probe: () => probeDevtools(port),
    diagnostics: async () =>
      "A debug build enables the devtools socket; a release build compiles the call away " +
      "(see e2e-android/README.md).",
    report: (line) => console.warn(line),
  });
}

export async function closeDevtools(port = DEFAULT_PORT): Promise<void> {
  await removeForward(port);
}
