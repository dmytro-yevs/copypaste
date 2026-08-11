import type { remote } from "webdriverio";

import { sleep, type Child } from "./process.js";

export type Browser = Awaited<ReturnType<typeof remote>>;

/**
 * The guard against a suite that passes while testing nothing.
 *
 * Every check below has failed at least once during development, and each
 * failure mode is silent: a WebView that loads but runs no JavaScript still
 * answers `execute` with `null`, and a dev server that is down produces a
 * WebKit error page whose DOM is a valid, queryable, entirely wrong document.
 */
export async function assertReallyRunning(
  browser: Browser,
  driver: Child,
): Promise<void> {
  const capabilities = browser.capabilities as { browserName?: string };
  assertTauriBrowserName(capabilities, process.platform);

  // An app stuck in a render loop never yields its main thread, so `execute`
  // does not return at all — the probe hangs instead of failing. Without this
  // wall clock the whole suite stalls until the CI job is killed, which reads
  // as an infrastructure problem rather than as the app being broken.
  await Promise.race([
    probeUntilMounted(browser, driver),
    new Promise<never>((_, reject) =>
      setTimeout(
        () =>
          reject(
            new Error(
              "the WebView never answered a script within 90s. Its main thread " +
                "is blocked — an infinite render loop does this — so the app is " +
                "running but cannot paint or respond.\n" +
                `tauri-driver log:\n${driver.log()}`,
            ),
          ),
        90_000,
      ),
    ),
  ]);
}

export function assertTauriBrowserName(
  capabilities: { browserName?: string },
  platform: NodeJS.Platform,
): void {
  const expected = platform === "win32" ? "webview2" : "wry";
  if (capabilities.browserName !== expected) {
    throw new Error(
      `expected the Tauri WebView ("${expected}"), got "${capabilities.browserName}". ` +
        `The session is not the app under test.`,
    );
  }
}

export function assertTauriBridge(
  probe: { bridge: boolean; url: string },
  bootstrapExpired: boolean,
): void {
  if (probe.bridge) return;
  // DMY-41: EdgeDriver can attach while WebView2 still exposes its initial page.
  if (!bootstrapExpired && probe.url === "about:blank") return;
  throw new Error(
    `window.__TAURI_INTERNALS__ is absent at ${probe.url} — the page is ` +
      `loaded outside the Tauri bridge, so no IPC is under test.`,
  );
}

async function probeUntilMounted(browser: Browser, driver: Child): Promise<void> {
  const deadline = Date.now() + 60_000;
  let last = "";
  for (;;) {
    const probe = (await browser.execute(function () {
      const root = document.getElementById("root");
      return {
        js: 2 + 2,
        bridge: "__TAURI_INTERNALS__" in window,
        nodes: document.querySelectorAll("*").length,
        rootChildren: root ? root.childElementCount : -1,
        text: document.body ? document.body.innerText.slice(0, 300) : "",
        url: location.href,
      };
    })) as {
      js: number;
      bridge: boolean;
      nodes: number;
      rootChildren: number;
      text: string;
      url: string;
    } | null;

    if (probe === null) {
      throw new Error(
        "the WebView returned null for a script that cannot return null — " +
          "JavaScript is not executing in the app.",
      );
    }
    if (probe.js !== 4) {
      throw new Error(`the WebView did not evaluate arithmetic: got ${probe.js}`);
    }
    assertTauriBridge(probe, Date.now() > deadline);
    if (probe.rootChildren > 0 && probe.nodes > 30) return;

    last = `url=${probe.url} nodes=${probe.nodes} rootChildren=${probe.rootChildren} text=${JSON.stringify(probe.text)}`;
    if (Date.now() > deadline) {
      throw new Error(
        `the app never mounted a UI. Last probe: ${last}\n` +
          `tauri-driver log:\n${driver.log()}`,
      );
    }
    await sleep(250);
  }
}
