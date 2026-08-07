import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

import type { TestProject } from "vitest/node";

import { PACKAGE, adb, isDebuggable, launchMain, shareText } from "./adb.js";
import { attachToApp } from "./app.js";
import { closeDevtools } from "./devtools.js";
import { freshNonce, ordinaryFor, secretFor } from "./fixtures.js";
import {
  SEARCH,
  clearField,
  gotoView,
  scrollListToTop,
  topRowIsMasked,
  waitFor,
  waitForRows,
  waitForText,
} from "./ui.js";

const OUT = process.env.HARNESS_OUT ?? "artifacts/android-ui";

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
    await clearField(app, SEARCH);

    // One share at a time, each confirmed on screen before the next. Two
    // `am start`s in a row reach IntakeActivity while the first is still
    // finishing and the second is dropped — silently, because `am` reports
    // that it started the activity either way.
    await shareText(secret);
    await launchMain();
    await waitFor(
      async () => {
        await scrollListToTop(app);
        return topRowIsMasked(app);
      },
      "the shared credential never became the newest row, masked",
      60_000,
    );

    await shareText(ordinary);
    await launchMain();
    await waitForText(app, ordinary, 60_000);
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
