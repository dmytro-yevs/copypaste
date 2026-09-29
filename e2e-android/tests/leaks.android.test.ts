import { afterAll, expect, inject, test } from "vitest";

import { PACKAGE } from "../src/harness/adb.js";
import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import { addItems, cleanUpItems } from "../src/harness/bridge.js";
import { ordinaryFor } from "../src/harness/fixtures.js";
import { accessibleSurface, expectNoFilesystemPath, expectNoRawError, outerHtml } from "../src/harness/leaks.js";
import { beforeAllWithEvidence } from "../src/harness/suite.js";
import { SEARCH, clearField, gotoView, openHistorySearch, reloadHistoryWith, waitForRows, waitForText } from "../src/harness/ui.js";

const clipping = ordinaryFor(inject("nonce"));
let app: AndroidApp;
let seeded: string[] = [];

beforeAllWithEvidence("leaks", async () => {
  app = await attachToApp();
  await gotoView(app, "Library");
  await openHistorySearch(app);
  await clearField(app, SEARCH);
  seeded = await addItems(app, [clipping]);
  await reloadHistoryWith(app, clipping);
  await waitForRows(app, 1);
  await waitForText(app, clipping);
}, 180_000);

afterAll(async () => { await cleanUpItems(app, seeded); await app?.detach(); });

test("an arbitrary clipping is visible on the device", async () => {
  expect(await outerHtml(app)).toContain(clipping);
});

test("no accessible string contains a filesystem path", async () => {
  const surface = await accessibleSurface(app);
  expect(surface).toContain(clipping);
  expectNoFilesystemPath(surface);
});

test("no raw transport wording is rendered anywhere", async () => {
  expectNoRawError(await outerHtml(app));
});

test("the path detector fails when it should", () => {
  for (const leak of [`/data/data/${PACKAGE}/databases/copypaste-v2.db`, `/data/user/0/${PACKAGE}/files`, "could not connect to /tmp/copypaste/daemon.sock", "C:\\Users\\someone\\AppData"]) {
    expect(() => expectNoFilesystemPath(leak)).toThrow();
  }
  expect(() => expectNoRawError("Connection refused")).toThrow();
});
