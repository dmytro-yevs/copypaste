import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

import type { TestProject } from "vitest/node";

import { PACKAGE, adb, isDebuggable, launchMain, shareText } from "./adb.js";
import { attachToApp, type AndroidApp } from "./app.js";
import { closeDevtools } from "./devtools.js";
import { freshNonce, ordinaryFor, secretFor } from "./fixtures.js";
import {
  SEARCH,
  resetHistoryFilters,
  gotoView,
  scrollListToTop,
  topRowIsMasked,
  visibleText,
  waitFor,
  waitForRows,
} from "./ui.js";

const OUT = process.env.HARNESS_OUT ?? "artifacts/android-ui";

const SHARE_ATTEMPTS = 3;

async function shareUntil(
  app: AndroidApp,
  text: string,
  arrived: () => Promise<boolean>,
): Promise<void> {
  for (let attempt = 1; attempt <= SHARE_ATTEMPTS; attempt += 1) {
    await shareText(text);
    await launchMain();
    try {
      await waitFor(arrived, "not yet", 25_000);
      return;
    } catch {
      /* dropped on the way in; send it again */
    }
  }
  throw new Error(`the shared clipping never reached the screen after ${SHARE_ATTEMPTS} attempts`);
}

export default async function setup(project: TestProject): Promise<() => Promise<void>> {
  const devices = (await adb("devices"))
    .split("\n")
    .slice(1)
    .filter((line) => /\tdevice$/.test(line));
  if (devices.length !== 1) {
    throw new Error(`expected one attached device, adb reports ${devices.length}`);
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

    // One share at a time, each confirmed on screen before the next. Two
    // `am start`s in a row reach IntakeActivity while the first is still
    // finishing and the second is dropped — silently, because `am` reports
    // that it started the activity either way.
    //
    // Confirmed and *re-sent* if it does not arrive: waiting longer does not
    // help a share that was dropped, and one that is dropped fails the whole
    // run in global setup. Measured on API 36 with a few dozen rows already in
    // the store, the second share is the one that goes missing.
    await shareUntil(app, secret, async () => {
      await scrollListToTop(app);
      return topRowIsMasked(app);
    });
    await shareUntil(app, ordinary, async () => {
      await scrollListToTop(app);
      return (await visibleText(app)).includes(ordinary);
    });
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
