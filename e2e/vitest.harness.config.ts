import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["src/harness/**/*.test.ts"],
    fileParallelism: false,
    testTimeout: 5_000,
    retry: 0,
  },
});
