import { afterEach, describe, expect, test } from "vitest";

import type { AndroidApp } from "../src/harness/app.js";
import { cleanUpItems, deleteItems } from "../src/harness/bridge.js";

/**
 * The real in-page loop, run in Node against a stub of the one global it
 * reaches. Asserting the classification alone would leave the loop — the half
 * that has to keep going after a refusal — unexercised.
 */
function appBackedBy(invoke: (command: string, args: { id: string }) => Promise<unknown>): AndroidApp {
  (globalThis as { window?: unknown }).window = { __TAURI_INTERNALS__: { invoke } };
  const page = { evaluate: async (fn: (arg: unknown) => unknown, arg: unknown) => fn(arg) };
  return { withPage: async (action: (p: unknown) => unknown) => action(page) } as unknown as AndroidApp;
}

/** What `delete_item` rejects with: a serialised `UiError`, never prose. */
const notFound = { code: "not_found", retryable: false };
const refused = { code: "internal", retryable: false };

afterEach(() => {
  delete (globalThis as { window?: unknown }).window;
});

describe("telling an id the store never held from one it refused to delete", () => {
  test("an id the screen deleted first is an outcome, not a failure", async () => {
    const report = await deleteItems(
      appBackedBy(async (_command, { id }) => {
        if (id === "gone") throw notFound;
        return true;
      }),
      ["kept", "gone"],
    );
    expect(report.deleted).toEqual(["kept"]);
    expect(report.alreadyGone).toEqual(["gone"]);
    expect(report.failed).toEqual([]);
  });

  // The defect: one `catch {}` covered both, so a cleanup that removed nothing
  // at all was indistinguishable from one with nothing left to remove — and the
  // credential `secretFor()` mints stayed in the history of every later run.
  test("a store that refused the delete fails the cleanup", async () => {
    await expect(
      deleteItems(
        appBackedBy(async () => {
          throw refused;
        }),
        ["a"],
      ),
    ).rejects.toThrow(/the store refused 1 of 1 fixture delete\(s\): a \(internal\)/);
  });

  test("every id is attempted even after one refuses", async () => {
    const seen: string[] = [];
    await expect(
      deleteItems(
        appBackedBy(async (_command, { id }) => {
          seen.push(id);
          if (id === "b") throw refused;
          if (id === "c") throw notFound;
          return true;
        }),
        ["a", "b", "c", "d"],
      ),
    ).rejects.toThrow(/1 of 4 fixture delete\(s\): b \(internal\)\. 2 deleted, 1 already gone/);
    expect(seen).toEqual(["a", "b", "c", "d"]);
  });

  test("a rejection that is not a UiError is still reported rather than swallowed", async () => {
    await expect(
      deleteItems(
        appBackedBy(async () => {
          throw new Error("Execution context was destroyed");
        }),
        ["a"],
      ),
    ).rejects.toThrow(/Execution context was destroyed/);
  });
});

describe("teardown", () => {
  test("a store refusal raised from teardown is the harness's own failure", async () => {
    await expect(
      cleanUpItems(
        appBackedBy(async () => {
          throw refused;
        }),
        ["a"],
      ),
    ).rejects.toThrow(/the store refused/);
  });

  // An app that is already gone is the failing test's reason, and replacing it
  // with this one would hide it.
  test("an app that can no longer be reached does not replace the test's reason", async () => {
    const unreachable = {
      withPage: async () => {
        throw new Error("Target closed");
      },
    } as unknown as AndroidApp;
    await expect(cleanUpItems(unreachable, ["a"])).resolves.toBeUndefined();
  });
});
