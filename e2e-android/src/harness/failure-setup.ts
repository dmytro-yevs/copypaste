import path from "node:path";

import { beforeEach, inject } from "vitest";

import { captureFailure } from "./evidence.js";
import { secretFor } from "./fixtures.js";
import { redactFromEvidence } from "./redact.js";

// The worker, not `global-setup.ts`: global setup runs in another process, so a
// value registered there never reaches the module instance that writes the file.
const nonce = inject("nonce");
if (nonce) redactFromEvidence(secretFor(nonce));

beforeEach((context) => {
  const suite = path.basename(context.task.file.name).replace(/\.android\.test\.ts$/, "");
  context.onTestFailed(async () => {
    await captureFailure(`${suite} ${context.task.name}`);
  });
});
