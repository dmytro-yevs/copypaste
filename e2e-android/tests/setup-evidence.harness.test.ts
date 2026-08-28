import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { afterEach, describe, expect, test, vi } from "vitest";

import { writeSetupFailureEvidence } from "../src/harness/setup-evidence.js";
import { runSuiteSetup } from "../src/harness/suite.js";

const PIXEL =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

describe("suite setup evidence", () => {
  let out = "";

  afterEach(() => {
    if (out) rmSync(out, { recursive: true, force: true });
    out = "";
  });

  test("writes deterministic artifacts before preserving the beforeAll failure", async () => {
    out = mkdtempSync(path.join(tmpdir(), "cp-setup-evidence-"));
    const primary = new Error("Devices screen never settled");
    const capture = vi.fn(() =>
      writeSetupFailureEvidence(
        "devices",
        "before-all",
        {
          shell: async () => ({
            url: "http://tauri.localhost/",
            route: "Devices",
          }),
          hierarchy: async () => ({
            nodes: [{ tag: "body", children: 1 }],
            truncated: false,
          }),
          screenshot: async () => PIXEL,
          logs: async () => ({ linesSampled: 20, androidRuntime: 0 }),
        },
        out,
      ),
    );

    await expect(
      runSuiteSetup(
        "devices",
        async () => {
          throw primary;
        },
        capture,
      ),
    ).rejects.toBe(primary);
    expect(capture).toHaveBeenCalledWith("devices", "before-all");
    const folder = path.join(out, "failures", "devices");
    expect(readdirSync(folder).sort()).toEqual(["before-all.json", "before-all.png"]);
    expect(
      readFileSync(path.join(folder, "before-all.png")).subarray(0, 8),
    ).toEqual(Buffer.from("89504e470d0a1a0a", "hex"));
    expect(JSON.parse(readFileSync(path.join(folder, "before-all.json"), "utf8"))).toMatchObject({
      suite: "devices",
      stage: "before-all",
      shell: { route: "Devices" },
    });
  });

  test("reports capture failure without replacing the primary setup error", async () => {
    const primary = new Error("Devices screen never settled");
    let caught: unknown;
    try {
      await runSuiteSetup(
        "devices",
        async () => {
          throw primary;
        },
        async () => {
          throw new Error(
            "required setup evidence screenshot was unavailable",
          );
        },
      );
    } catch (error) {
      caught = error;
    }

    expect(caught).toBeInstanceOf(AggregateError);
    expect((caught as AggregateError).errors[0]).toBe(primary);
    expect((caught as AggregateError).errors[1]).toMatchObject({
      message: "required setup evidence screenshot was unavailable",
    });
  });

  test("successful setup captures no failure artifact", async () => {
    out = mkdtempSync(path.join(tmpdir(), "cp-setup-evidence-"));
    const capture = vi.fn();

    await runSuiteSetup("devices", async () => undefined, capture);

    expect(capture).not.toHaveBeenCalled();
    expect(readdirSync(out)).toEqual([]);
  });

  test("every Android suite setup uses the evidence wrapper", () => {
    const tests = fileURLToPath(new URL(".", import.meta.url));
    const direct = readdirSync(tests)
      .filter((file) => file.endsWith(".android.test.ts"))
      .filter((file) => /\bbeforeAll\(/.test(readFileSync(path.join(tests, file), "utf8")));

    expect(direct).toEqual([]);
  });
});
