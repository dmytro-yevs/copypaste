/**
 * The push channel: `Method::Watch` → `service::push` → two window events →
 * `usePush`.
 *
 * Two things are asserted that a unit test cannot reach. First, that an event
 * really crosses the bridge: the test registers **its own** listener through
 * the same event plugin the app uses, so what it observes is the delivery, not
 * a mock of one. Second, that the delivery is what updates the screen — the row
 * appears well inside the 3 s poll that would otherwise have fetched it.
 *
 * The third test is the obligation `usePush` is written around: push
 * *accelerates* the poll and must never replace it. A daemon that dies takes
 * the subscription with it, and the window must degrade to polling rather than
 * to a screen that is silently frozen at yesterday's clipboard.
 */
import { afterAll, beforeAll, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import { expectNoFilesystemPath, accessibleSurface } from "../src/harness/leaks.js";
import { rowCount, waitForRows, waitForText } from "../src/harness/ui.js";

/** `service::push::EVENT_CHANGED`, asserted on the Rust side by a test that
 *  names this string. Duplicated here on purpose: a test that imported it
 *  could not catch it changing. */
const EVENT_CHANGED = "copypaste://changed";
const EVENT_PUSH_STATE = "copypaste://push-state";

/** `POLL_ACTIVE_MS` from `lib/layout.ts` — what a poll-only build would cost. */
const POLL_ACTIVE_MS = 3000;

interface ChangeEvent {
  event: string;
  payload: { topic: string; item_count: number };
}

interface PushStateEvent {
  event: string;
  payload: { live: boolean };
}

let app: App;

beforeAll(async () => {
  app = await startApp({ seed: ["push baseline"] });
  await waitForRows(app.browser, 1);
  await subscribe();
  await subscribePushState();
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

/**
 * Listen for the change event from the test itself.
 *
 * Through `__TAURI_INTERNALS__` rather than the app's own module graph, which
 * is bundled and unreachable from here — but it is the same `plugin:event`
 * command `@tauri-apps/api`'s `listen` calls, so a delivery observed here is a
 * delivery the app's own subscriber saw too.
 */
async function subscribe(): Promise<void> {
  const result = (await app.browser.executeAsync(
    (event: string, done: (value: unknown) => void) => {
      const w = window as unknown as {
        __TAURI_INTERNALS__: {
          invoke: (command: string, args: unknown) => Promise<unknown>;
          transformCallback: (fn: (payload: unknown) => void) => number;
        };
        __e2ePush?: unknown[];
      };
      w.__e2ePush = [];
      const handler = w.__TAURI_INTERNALS__.transformCallback((payload) => {
        w.__e2ePush!.push(payload);
      });
      w.__TAURI_INTERNALS__
        .invoke("plugin:event|listen", {
          event,
          target: { kind: "Any" },
          handler,
        })
        .then(
          () => done({ ok: true }),
          (error: unknown) => done({ ok: false, error: String(error) }),
        );
    },
    EVENT_CHANGED,
  )) as { ok: boolean; error?: string };

  if (!result.ok) {
    throw new Error(`could not subscribe to ${EVENT_CHANGED}: ${result.error}`);
  }
}

async function subscribePushState(): Promise<void> {
  const result = (await app.browser.executeAsync(
    (event: string, done: (value: unknown) => void) => {
      const w = window as unknown as {
        __TAURI_INTERNALS__: {
          invoke: (command: string, args: unknown) => Promise<unknown>;
          transformCallback: (fn: (payload: unknown) => void) => number;
        };
        __e2ePushState?: unknown[];
      };
      w.__e2ePushState = [];
      const handler = w.__TAURI_INTERNALS__.transformCallback((payload) => {
        w.__e2ePushState!.push(payload);
      });
      w.__TAURI_INTERNALS__
        .invoke("plugin:event|listen", {
          event,
          target: { kind: "Any" },
          handler,
        })
        .then(
          () => done({ ok: true }),
          (error: unknown) => done({ ok: false, error: String(error) }),
        );
    },
    EVENT_PUSH_STATE,
  )) as { ok: boolean; error?: string };

  if (!result.ok) {
    throw new Error(`could not subscribe to ${EVENT_PUSH_STATE}: ${result.error}`);
  }
}

async function received(): Promise<ChangeEvent[]> {
  return (await app.browser.execute(
    () => (window as unknown as { __e2ePush?: ChangeEvent[] }).__e2ePush ?? [],
  )) as ChangeEvent[];
}

async function receivedPushStates(): Promise<PushStateEvent[]> {
  return (await app.browser.execute(
    () =>
      (window as unknown as { __e2ePushState?: PushStateEvent[] }).__e2ePushState ??
      [],
  )) as PushStateEvent[];
}

test("a change event actually arrives in the WebView", async () => {
  const before = (await received()).length;
  await app.daemon.add("an item that should announce itself");

  await app.browser.waitUntil(async () => (await received()).length > before, {
    timeout: 15_000,
    interval: 200,
    timeoutMsg:
      "no copypaste://changed event reached the WebView — the daemon's watch " +
      "channel, the bridge's subscription, or the event name is broken",
  });

  const event = (await received()).at(-1)!;
  expect(event.event).toBe(EVENT_CHANGED);
  expect(event.payload.topic).toBe("items");
  expect(event.payload.item_count).toBeGreaterThan(0);
});

test("the list updates without waiting for the poll", async () => {
  // Letters only: a bare millisecond timestamp is thirteen digits, which the
  // detector reads as an account number and withholds — the row then renders
  // as "Sensitive content hidden" and the wait fails for a reason that has
  // nothing to do with push. Found by this test flaking on one run in two.
  const needle = "a clipping delivered by the change stream";

  // Timed inside the page: a WebDriver round trip per sample would add its own
  // latency to the number under test. `t0` is taken before the CLI runs, so
  // the measurement includes the write itself and is a bound, never a
  // flattering one.
  await app.browser.execute(function (text: string) {
    const w = window as unknown as { __e2eSeen?: number | null; __e2eT0?: number };
    w.__e2eSeen = null;
    w.__e2eT0 = performance.now();
    const observer = new MutationObserver(function () {
      if (w.__e2eSeen === null && document.body.innerText.indexOf(text) !== -1) {
        w.__e2eSeen = performance.now();
        observer.disconnect();
      }
    });
    observer.observe(document.body, {
      subtree: true,
      childList: true,
      characterData: true,
    });
  }, needle);

  await app.daemon.add(needle);
  await waitForText(app.browser, needle, 30_000);

  const latency = (await app.browser.execute(() => {
    const w = window as unknown as { __e2eSeen: number | null; __e2eT0: number };
    return w.__e2eSeen === null ? null : w.__e2eSeen - w.__e2eT0;
  })) as number | null;

  expect(latency, "the row appeared without a DOM mutation").not.toBeNull();
  // A poll-only build would average half the interval and often exceed this;
  // push delivers in tens of milliseconds plus one render. The margin is what
  // absorbs a `copypaste add` process spawn on a loaded machine.
  expect(latency!).toBeLessThan(POLL_ACTIVE_MS - 500);
});

test("a daemon that dies degrades to polling, not to a broken screen", async () => {
  const { browser } = app;
  await app.daemon.kill();

  // The rows that were already fetched stay: a failed background poll must not
  // throw away what the user is reading.
  await browser.waitUntil(
    async () =>
      (await receivedPushStates()).some(
        (event) => event.event === EVENT_PUSH_STATE && !event.payload.live,
      ),
    {
      timeout: 30_000,
      timeoutMsg: "the push state never reported that the subscription was lost",
    },
  );
  expect(await rowCount(browser)).toBeGreaterThan(0);
  expectNoFilesystemPath(await accessibleSurface(browser), app.daemon.dataHome);

  await app.daemon.restart();
  const afterRestart = "arrived while the subscription was down";
  await app.daemon.add(afterRestart);

  // No reload, no button: the poll is the backstop, and it is what has to
  // recover the window (INV-2's cadence, `pollInterval`'s reason for existing).
  await waitForText(browser, afterRestart, 45_000);

  // The subscription reports its recovered state independently of the history
  // query. The row assertion above remains the no-reload recovery check.
  await browser.waitUntil(
    async () =>
      (await receivedPushStates()).some(
        (event) => event.event === EVENT_PUSH_STATE && event.payload.live,
      ),
    {
      timeout: 30_000,
      interval: 500,
      timeoutMsg: "the push state stayed degraded after the service came back",
    },
  );
}, 120_000);
