import path from "node:path";

import { beforeEach, inject } from "vitest";

import { captureFailure } from "./evidence.js";
import { secretFor } from "./fixtures.js";

// Minting registers (see `fixtures.ts`), and every suite that seeds a credential
// mints its own. This covers the one nothing here mints: `global-setup.ts` seeds
// the run's credential from another process, so no worker would otherwise hold
// it, and it is on screen for every suite.
const nonce = inject("nonce");
if (nonce) secretFor(nonce);

beforeEach((context) => {
  const suite = path.basename(context.task.file.name).replace(/\.android\.test\.ts$/, "");
  context.onTestFailed(async () => {
    await captureFailure(`${suite} ${context.task.name}`);
  });
});
