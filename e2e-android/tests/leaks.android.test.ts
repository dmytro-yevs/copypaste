import { afterAll, beforeAll, expect, inject, test } from "vitest";

import { PACKAGE } from "../src/harness/adb.js";
import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import { addItems, cleanUpItems } from "../src/harness/bridge.js";
import { ordinaryFor, secretFor } from "../src/harness/fixtures.js";
import {
  accessibleSurface,
  expectNoFilesystemPath,
  expectNoRawError,
  expectSecretAbsent,
  outerHtml,
} from "../src/harness/leaks.js";
import {
  MASKED_ROW,
  SEARCH,
  clearField,
  count,
  gotoView,
  reloadHistoryWith,
  waitForRows,
  waitForText,
} from "../src/harness/ui.js";

const nonce = inject("nonce");
const leakNonce = String((Number(nonce) + 1) % 1_000_000_000).padStart(9, "0");
const secret = secretFor(leakNonce);
const ordinary = ordinaryFor(leakNonce);

let app: AndroidApp;
let seeded: string[] = [];

beforeAll(async () => {
  app = await attachToApp();
  await gotoView(app, "History");
  await clearField(app, SEARCH);
  seeded = await addItems(app, [secret, ordinary]);
  await reloadHistoryWith(app, ordinary);
  await waitForRows(app, 1);
  await waitForText(app, ordinary);
}, 180_000);

afterAll(async () => {
  await cleanUpItems(app, seeded);
  await app?.detach();
});

test("the shared credential was classified as sensitive on the device", async () => {
  expect(await count(app, MASKED_ROW)).toBeGreaterThan(0);
});

test("a sensitive item's plaintext is absent from the document, not merely covered", async () => {
  expect(await outerHtml(app)).toContain(ordinary);
  await expectSecretAbsent(app, secret);
});

test("no accessible string contains a filesystem path (INV-12)", async () => {
  const surface = await accessibleSurface(app);
  // An empty surface satisfies every negative assertion below it, and a
  // detached frame or a blank screen produces exactly that.
  expect(surface).toContain(ordinary);
  expectNoFilesystemPath(surface);
});

test("no raw transport wording is rendered anywhere", async () => {
  expectNoRawError(await outerHtml(app));
});

test("the path detector fails when it should", () => {
  for (const leak of [
    `/data/data/${PACKAGE}/databases/copypaste-v2.db`,
    `/data/user/0/${PACKAGE}/files`,
    "could not connect to /tmp/copypaste/daemon.sock",
    "C:\\Users\\someone\\AppData",
  ]) {
    expect(() => expectNoFilesystemPath(leak)).toThrow();
  }
  expect(() => expectNoRawError("Connection refused")).toThrow();
});
