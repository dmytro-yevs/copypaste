import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { afterEach, describe, expect, test } from "vitest";

import { writeAttachment } from "../src/harness/attachment.js";
import { fixtureMarker, ordinaryFor, secretFor } from "../src/harness/fixtures.js";
import { writeRedacted } from "../src/harness/redact.js";

describe("fixture markers", () => {
  test("do not contain card-shaped wall-clock numbers", () => {
    const hostedFailure = `settings-${1786494647418}`;

    expect(hostedFailure).toMatch(/[0-9]{13,19}/);
    expect(fixtureMarker("settings", 1786494647418)).not.toMatch(/[0-9]{13,19}/);
  });

  // Registering a fixture as it is minted must not change what it mints: the
  // credential only proves anything while it trips `aws_access_key`, and the
  // ordinary clipping is waited for by text.
  test("still mint exactly the strings the device is seeded with", () => {
    expect(secretFor("564357103")).toBe("AKIAHARNESS564357103");
    expect(secretFor("564357103")).toMatch(/^AKIA[0-9A-Z]{16}$/);
    expect(ordinaryFor("564357103")).toBe("an ordinary clipping HARNESS564357103");
  });
});

describe("failure evidence", () => {
  let dir = "";

  afterEach(() => {
    if (dir) rmSync(dir, { recursive: true, force: true });
    dir = "";
  });

  /** Through `writeRedacted`, which is the call `captureFailure` publishes with:
   *  asserting on the returned string would prove nothing about the file. */
  function published(value: unknown): string {
    dir = mkdtempSync(path.join(tmpdir(), "cp-evidence-"));
    const file = path.join(dir, "failures", "leaks-plaintext.json");
    writeRedacted(file, value);
    return readFileSync(file, "utf8");
  }

  // The run's credential and the one `leaks.android.test.ts` derives from its
  // own nonce, in the shape a failing INV row would carry them.
  test("publishes neither seeded credential, nor a value they can be rebuilt from", () => {
    const runNonce = "000000042";
    const leakNonce = "000000043";
    const runSecret = secretFor(runNonce);
    const leakSecret = secretFor(leakNonce);

    const written = published({
      list: {
        totalSize: 4020,
        scrollTop: 2400,
        rows: [
          { id: "r1", start: 0, height: 67, text: `revealed ${runSecret}` },
          { id: "r2", start: 67, height: 67, text: `revealed ${leakSecret}` },
          { id: "r3", start: 134, height: 67, text: ordinaryFor(leakNonce) },
        ],
      },
    });

    for (const value of [runSecret, leakSecret, runNonce, leakNonce]) {
      expect(written).not.toContain(value);
    }
    expect(written).not.toMatch(/AKIA[0-9A-Z]{16}/);
  });

  test("redacts a credential this harness never minted", () => {
    const written = published({ rows: [{ text: "arrived from the device AKIAZZZZ0123456789AB" }] });

    expect(written).not.toContain("AKIAZZZZ0123456789AB");
    expect(written).not.toMatch(/AKIA[0-9A-Z]{16}/);
  });

  // What INV-1 and INV-5 are diagnosed from; redaction must not cost it.
  test("keeps the geometry and the row text an invariant failure is read from", () => {
    const written = published({
      list: {
        totalSize: 4020,
        scrollTop: 2400,
        clientHeight: 352,
        rows: [
          { id: "r1", start: 2412, height: 67, text: "render-mabc item 41 short" },
          { id: "r2", start: 2479, height: 67, text: "render-mabc item 42 long long long" },
        ],
      },
    });

    const parsed = JSON.parse(written);
    expect(parsed.list).toMatchObject({ totalSize: 4020, scrollTop: 2400, clientHeight: 352 });
    expect(parsed.list.rows.map((row: { start: number }) => row.start)).toEqual([2412, 2479]);
    expect(parsed.list.rows[1].text).toContain("long long");
    expect(parsed.list.rows[0].text).toContain("short");
  });
});

/**
 * The directory `android-emulator.yml` uploads whole, assembled from the calls
 * that fill it. Asserting on one file is what let `attachment.json` publish the
 * nonce: nine digits rebuilds `AKIAHARNESS<nonce>` and, through `leakNonce`,
 * the leak suite's credential as well.
 */
describe("the uploaded android-ui directory", () => {
  let out = "";

  afterEach(() => {
    if (out) rmSync(out, { recursive: true, force: true });
    out = "";
  });

  function filesUnder(dir: string): string[] {
    return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
      const full = path.join(dir, entry.name);
      return entry.isDirectory() ? filesUnder(full) : [full];
    });
  }

  test("contains no credential, no nonce and nothing either rebuilds from", () => {
    const nonce = "564357103";
    const leakNonce = String((Number(nonce) + 1) % 1_000_000_000).padStart(9, "0");
    out = mkdtempSync(path.join(tmpdir(), "cp-android-ui-"));

    // Both writers, as the leg calls them.
    writeAttachment(out, {
      package: "com.copypaste.app.debug",
      pid: 4157,
      socket: "webview_devtools_remote_4157",
      url: "http://tauri.localhost/",
      title: "CopyPaste",
      version: { Browser: "Chrome/133.0.6943.137", "Android-Package": "com.copypaste.app.debug" },
    });
    writeRedacted(path.join(out, "failures", "leaks-plaintext.json"), {
      rows: [
        { text: `revealed ${secretFor(nonce)}` },
        { text: `revealed ${secretFor(leakNonce)}` },
        { text: ordinaryFor(nonce) },
      ],
    });

    const files = filesUnder(out);
    // An empty walk would satisfy every assertion below it.
    expect(files.map((file) => path.relative(out, file).replace(/\\/g, "/")).sort()).toEqual([
      "attachment.json",
      "failures/leaks-plaintext.json",
    ]);

    const uploaded = files.map((file) => readFileSync(file, "utf8")).join("\n");
    for (const value of [secretFor(nonce), secretFor(leakNonce), nonce, leakNonce]) {
      expect(uploaded).not.toContain(value);
    }
    expect(uploaded).not.toMatch(/AKIA[0-9A-Z]{16}/);
    // Still the record of which WebView the run drove.
    expect(JSON.parse(readFileSync(path.join(out, "attachment.json"), "utf8"))).toMatchObject({
      package: "com.copypaste.app.debug",
      socket: "webview_devtools_remote_4157",
    });
  });

  // The claim `redact.ts` makes about itself, held to the source: a second
  // direct writer is how the nonce reached an upload in the first place.
  test("is written only through the redacting path", () => {
    const harness = fileURLToPath(new URL("../src/harness", import.meta.url));
    const writers = filesUnder(harness).filter((file) =>
      /\bwriteFileSync\b/.test(readFileSync(file, "utf8")),
    );

    expect(writers.map((file) => path.basename(file))).toEqual(["redact.ts"]);
  });
});
