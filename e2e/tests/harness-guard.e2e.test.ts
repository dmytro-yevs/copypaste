import { expect, test } from "vitest";

import { startApp } from "../src/harness/app.js";
import { cliBinary } from "../src/harness/env.js";

/**
 * The suite's own guard. Everything else here asserts against a running app,
 * and all of those assertions would vacuously pass — or never run — if the
 * harness quietly failed to launch one. This pins the opposite: given a binary
 * that is emphatically not the app, starting must throw rather than hand back
 * a session that tests nothing.
 */
test("startApp refuses a process that is not the app under test", async () => {
  const previous = process.env.COPYPASTE_UI_BIN;
  process.env.COPYPASTE_UI_BIN = cliBinary();
  try {
    await expect(startApp()).rejects.toThrow();
  } finally {
    if (previous === undefined) delete process.env.COPYPASTE_UI_BIN;
    else process.env.COPYPASTE_UI_BIN = previous;
  }
}, 240_000);
