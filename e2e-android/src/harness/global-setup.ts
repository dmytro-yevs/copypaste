import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

import type { TestProject } from "vitest/node";

import { PACKAGE, adb, isDebuggable, launchMain, shareText } from "./adb.js";
import { attachToApp, type AndroidApp } from "./app.js";
import { storedItems } from "./bridge.js";
import { closeDevtools } from "./devtools.js";
import { freshNonce, ordinaryFor, secretFor } from "./fixtures.js";
import {
  resetHistoryFilters,
  gotoView,
  scrollListToTop,
  visibleText,
  waitFor,
  waitForRows,
} from "./ui.js";

const OUT = process.env.HARNESS_OUT ?? "artifacts/android-ui";

async function shareAndWait(
  app: AndroidApp,
  text: string,
  arrived: () => Promise<boolean>,
): Promise<void> {
  await shareText(text);
  await launchMain();
  await waitFor(arrived, "the shared clipping never reached the store", 60_000);
}

export default async function setup(project: TestProject): Promise<() => Promise<void>> {
  const attached = (await adb("devices"))
    .split("\n")
    .slice(1)
    .filter((line) => /\tdevice$/.test(line))
    .map((line) => line.split("\t", 1)[0]!);
  const selected = process.env.ANDROID_SERIAL;
  if (selected ? !attached.includes(selected) : attached.length !== 1) {
    throw new Error(
      selected
        ? `ANDROID_SERIAL ${selected} is not an attached device`
        : `expected one attached device, adb reports ${attached.length}; set ANDROID_SERIAL`,
    );
  }

  // Stated as a precondition rather than discovered as a timeout: wry compiles
  // `setWebContentsDebuggingEnabled` out of a release build entirely
  // (`#[cfg(any(debug_assertions, feature = "devtools"))]`), so on the APK
  // people install there is no socket to attach to — and there must not be.
  if (!(await isDebuggable())) {
    throw new Error(
      `${PACKAGE} is not debuggable; the WebView publishes no devtools socket and this harness cannot run`,
    );
  }

  const nonce = freshNonce();
  project.provide("nonce", nonce);
  const secret = secretFor(nonce);
  const ordinary = ordinaryFor(nonce);

  await launchMain();
  const app = await attachToApp();
  try {
    // The activity is `singleTask` and comes back exactly where it was left,
    // including a search filter from a previous run that would hide this run's
    // fixtures and read as "the item was never ingested".
    await gotoView(app, "History");
    await resetHistoryFilters(app);

    // One share at a time, each confirmed in the store before the next. Two
    // `am start`s in a row reach IntakeActivity while the first is still
    // finishing and the second is dropped — silently, because `am` reports
    // that it started the activity either way.
    // The store observation is not retried input: a dropped intent fails this
    // run instead of silently sending a second clipping and reporting green.
    const before = new Set((await storedItems(app)).map((item) => item.id));
    await shareAndWait(app, secret, async () =>
      (await storedItems(app)).some(
        (item) => !before.has(item.id) && item.is_sensitive,
      ),
    );
    await shareAndWait(app, ordinary, async () =>
      (await storedItems(app)).some((item) => item.content === ordinary),
    );
    await scrollListToTop(app);
    await waitFor(
      async () => (await visibleText(app)).includes(ordinary),
      "the shared ordinary clipping reached the store but not the screen",
      60_000,
    );
    await waitForRows(app, 2, 60_000);

    mkdirSync(OUT, { recursive: true });
    writeFileSync(
      path.join(OUT, "attachment.json"),
      JSON.stringify(
        {
          package: PACKAGE,
          pid: app.endpoint.pid,
          socket: app.endpoint.socket,
          url: app.page.url(),
          title: await app.page.title(),
          version: app.endpoint.version,
          nonce,
        },
        null,
        2,
      ),
    );
  } finally {
    await app.detach();
  }

  return async () => {
    await closeDevtools();
  };
}
