import { test, expect } from "playwright/test";

// Proves the browser → /__ipc → daemon socket path returns REAL data, and
// captures a screenshot of the live app for the agent to read.
test("bridge reaches the live daemon and the app renders", async ({
  request,
  page,
}) => {
  // 1. Deterministic data-path check: history_page must return ok:true.
  const res = await request.post("http://localhost:1420/__ipc", {
    data: { method: "history_page", params: { limit: 1, offset: 0 } },
  });
  const reply = await res.json();
  expect(reply.ok).toBe(true);

  // 2. Render the real app in bridge mode and snapshot it.
  await page.goto("http://localhost:1420/?bridge=1");
  await page.waitForLoadState("networkidle");
  await page.screenshot({
    path: "e2e/dev-drive/__artifacts__/app.png",
    fullPage: true,
  });
});
