import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["**/*.android.test.ts"],
    globalSetup: ["./src/harness/global-setup.ts"],
    setupFiles: ["./src/harness/failure-setup.ts"],
    // One device, one `adb forward`, one WebView. Two files attaching at once
    // would race on the forward and on the app's foreground state.
    fileParallelism: false,
    testTimeout: 60_000,
    hookTimeout: 180_000,
  },
});
