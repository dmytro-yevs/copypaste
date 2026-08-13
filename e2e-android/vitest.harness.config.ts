import { defineConfig } from "vitest/config";

// The harness's own contracts, with no device and no global setup: a bound that
// is shorter than a cold start, a relaunch that is never re-resolved and a
// timeout with nothing to act on are all decisions, and every one of them cost
// a hosted emulator leg to find.
export default defineConfig({
  test: {
    include: ["**/*.harness.test.ts"],
  },
});
