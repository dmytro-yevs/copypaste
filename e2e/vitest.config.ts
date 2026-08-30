import { defineConfig } from "vitest/config";

const shard = process.env.COPYPASTE_E2E_SHARD;
if (shard !== undefined && !/^[1-9]\d*\/[1-9]\d*$/.test(shard)) {
  throw new Error(`COPYPASTE_E2E_SHARD must be <index>/<count>, got ${shard}`);
}

// WebKitWebDriver refuses a second concurrent session ("Maximum number of
// active sessions"), and each test file drives its own app instance, so files
// must not overlap. This is a correctness constraint, not a tuning choice.
export default defineConfig({
  test: {
    // The layer this suite is, in every reporter line it prints: the caveats
    // in docs/rewrite/testing-policy.md are worth nothing if CI output reads
    // as platform coverage.
    name:
      process.platform === "win32"
        ? "native desktop (WebView2, Windows)"
        : "browser (WebKitGTK, Linux)",
    include: ["tests/**/*.e2e.test.ts", "src/**/*.test.ts"],
    exclude:
      process.platform === "win32" ? [] : ["tests/**/*.windows.e2e.test.ts"],
    globalSetup: ["./src/harness/global-setup.ts"],
    fileParallelism: false,
    testTimeout: 60_000,
    hookTimeout: 240_000,
    teardownTimeout: 30_000,
    retry: 0,
    ...(shard ? { shard } : {}),
    reporters: ["default"],
  },
});
