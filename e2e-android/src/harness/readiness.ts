/**
 * Wait for a device condition on a bounded adb-process budget.
 *
 * Device-free, like `attach.ts`: every other harness module reaches `adb.ts`,
 * which shells out at import time.
 *
 * The discovery loop in `devtools.ts` polled every second for the whole timeout
 * and read every failure as "not running yet". Both halves were wrong. A poll
 * spends two adb processes, so a 90s wait could launch 180 of them against an
 * emulator that was never going to answer; the budget here is counted in
 * processes and the caller declares what one probe costs. And a probe now
 * returns a typed outcome, so a failure that no wait can fix propagates on the
 * attempt that found it instead of being retried to the deadline.
 *
 * The PowerShell twin is `scripts/release/windows-readiness-lib.ps1`. The
 * vocabulary is deliberately the same; a PowerShell script and an ES module
 * cannot share one.
 */

export type Probe<T> =
  | { kind: "ready"; value: T }
  | { kind: "not-ready"; why: string }
  | { kind: "transient"; why: string }
  | { kind: "invariant"; why: string };

export interface ReadinessSchedule {
  attempts: number;
  delays: number[];
  processes: number;
  limitedBy: "timeout" | "process-budget";
}

export interface ScheduleRequest {
  timeoutMs: number;
  processBudget?: number;
  processesPerProbe?: number;
  firstDelayMs?: number;
  maxDelayMs?: number;
}

/**
 * Exponential backoff to the deadline, truncated by however many probes the
 * process budget pays for. `limitedBy` is the half that ran out first: a run
 * that stopped probing early must not report itself as having waited the whole
 * timeout.
 */
export function readinessSchedule({
  timeoutMs,
  processBudget = 0,
  processesPerProbe = 0,
  firstDelayMs = 500,
  maxDelayMs = 4_000,
}: ScheduleRequest): ReadinessSchedule {
  if (timeoutMs < 0) throw new Error("a readiness timeout cannot be negative");
  if (processesPerProbe < 0) throw new Error("a probe cannot cost a negative number of processes");
  if (firstDelayMs <= 0 || maxDelayMs < firstDelayMs) {
    throw new Error("readiness backoff must start above zero and may not exceed its own cap");
  }

  let maxAttempts = 0;
  if (processesPerProbe > 0) {
    if (processBudget < processesPerProbe) {
      throw new Error(
        `a process budget of ${processBudget} cannot pay for one probe costing ${processesPerProbe}`,
      );
    }
    maxAttempts = Math.floor(processBudget / processesPerProbe);
  }

  const delays: number[] = [];
  let elapsed = 0;
  let delay = firstDelayMs;
  while (elapsed < timeoutMs) {
    if (maxAttempts > 0 && delays.length + 1 >= maxAttempts) break;
    const step = Math.min(delay, timeoutMs - elapsed);
    delays.push(step);
    elapsed += step;
    delay = Math.min(maxDelayMs, delay * 2);
  }

  const attempts = delays.length + 1;
  return {
    attempts,
    delays,
    processes: attempts * processesPerProbe,
    limitedBy: elapsed < timeoutMs ? "process-budget" : "timeout",
  };
}

export interface WaitRequest<T> extends ScheduleRequest {
  description: string;
  probe: (attempt: number) => Promise<Probe<T>>;
  /** Read only when the wait has already failed, to say what the device showed. */
  diagnostics?: () => Promise<string>;
  sleep?: (ms: number) => Promise<void>;
  report?: (line: string) => void;
}

export async function waitForReadiness<T>(request: WaitRequest<T>): Promise<T> {
  const {
    description,
    probe,
    diagnostics,
    sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms)),
    report = () => undefined,
    processBudget = 0,
    processesPerProbe = 0,
  } = request;
  const schedule = readinessSchedule(request);
  const started = Date.now();
  let last = "the probe was never run";
  let transients = 0;
  let spent = 0;

  for (let attempt = 1; attempt <= schedule.attempts; attempt++) {
    const outcome = await probe(attempt);
    spent += processesPerProbe;
    switch (outcome?.kind) {
      case "ready":
        return outcome.value;
      case "invariant":
        throw new Error(
          `${description} cannot become ready: ${outcome.why}. ` +
            `Observed after ${attempt} probe(s) and ${Date.now() - started} ms.`,
        );
      case "transient":
        transients++;
        last = `transient: ${outcome.why}`;
        report(`readiness: ${description} saw a transient failure on probe ${attempt}: ${outcome.why}`);
        break;
      case "not-ready":
        last = `not ready: ${outcome.why}`;
        break;
      default:
        throw new Error(
          `${description} probe returned no outcome; it must return a ready, not-ready, transient or invariant Probe`,
        );
    }
    if (attempt < schedule.attempts) await sleep(schedule.delays[attempt - 1]!);
  }

  const detail = diagnostics ? await diagnostics().catch((error: unknown) => String(error)) : "";
  const bound =
    schedule.limitedBy === "process-budget"
      ? `its process budget of ${processBudget} ran out after ${schedule.attempts} probe(s) costing ${processesPerProbe} each`
      : `it timed out after ${request.timeoutMs} ms and ${schedule.attempts} probe(s)`;
  throw new Error(
    `${description} never became ready: ${bound}. ` +
      `Spent ${spent} process(es), saw ${transients} transient failure(s). Last outcome: ${last}.` +
      (detail ? ` Diagnostics:\n${detail}` : ""),
  );
}
