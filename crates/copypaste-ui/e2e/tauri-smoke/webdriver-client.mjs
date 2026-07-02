/**
 * webdriver-client.mjs — minimal hand-rolled W3C WebDriver HTTP client.
 *
 * Used by run.mjs's Tier-2 probes to talk to a running `tauri-driver`
 * WebDriver-protocol server (https://www.w3.org/TR/webdriver2/) over plain
 * HTTP, using Node's built-in `fetch` (Node >=18, available since AbortSignal
 * .timeout landed in 17.3). Deliberately NOT a dependency on webdriverio /
 * selenium-webdriver: crates/copypaste-ui/package.json is scoped (for this
 * change, CopyPaste-g27b.13.3) to test:* script edits only — no new
 * devDependencies. See run.mjs's header comment for the full picture of
 * where this actually runs vs. skips.
 *
 * Only the handful of WebDriver commands run.mjs actually needs are
 * implemented here — this is not a general-purpose client.
 */

export class WebDriverCommandError extends Error {}

export class WebDriverSession {
  constructor(baseUrl, sessionId) {
    this.baseUrl = baseUrl;
    this.sessionId = sessionId;
  }

  /** Poll GET /status until the WebDriver server responds OK, or timeout. */
  static async waitForServerReady(baseUrl, timeoutMs) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      try {
        const res = await fetch(`${baseUrl}/status`, {
          signal: AbortSignal.timeout(1500),
        });
        if (res.ok) return true;
      } catch {
        // Server not up yet (connection refused / timed out) — retry until
        // the deadline.
      }
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
    return false;
  }

  /** POST /session — create a new WebDriver session. `capabilities` is the
   * full W3C capabilities object, e.g.
   * `{ alwaysMatch: { "tauri:options": { application: "/path/to/App" } } }`
   * (the `tauri:options.application` vendor capability is how tauri-driver
   * launches and attaches to the packaged app itself — there is no separate
   * "navigate to URL" step for a Tauri app the way there is for a browser). */
  static async create(baseUrl, capabilities, { timeoutMs = 20000 } = {}) {
    const res = await fetch(`${baseUrl}/session`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ capabilities }),
      signal: AbortSignal.timeout(timeoutMs),
    });
    const body = await res.json().catch(() => null);
    if (!res.ok || !body?.value?.sessionId) {
      throw new WebDriverCommandError(
        `WebDriver session creation failed (HTTP ${res.status}): ${JSON.stringify(body)}`,
      );
    }
    return new WebDriverSession(baseUrl, body.value.sessionId);
  }

  async _command(method, subPath, body) {
    const res = await fetch(`${this.baseUrl}/session/${this.sessionId}${subPath}`, {
      method,
      headers: body !== undefined ? { "content-type": "application/json" } : undefined,
      body: body !== undefined ? JSON.stringify(body) : undefined,
      signal: AbortSignal.timeout(15000),
    });
    const parsed = await res.json().catch(() => null);
    if (!res.ok) {
      throw new WebDriverCommandError(
        `WebDriver ${method} ${subPath} failed (HTTP ${res.status}): ${JSON.stringify(parsed)}`,
      );
    }
    return parsed?.value;
  }

  /** POST /session/:id/execute/sync */
  executeScript(script, args = []) {
    return this._command("POST", "/execute/sync", { script, args });
  }

  /** POST /session/:id/execute/async — `script` receives the completion
   * callback as its LAST argument (the standard W3C async-script
   * convention: `arguments[arguments.length - 1]`). */
  executeAsyncScript(script, args = []) {
    return this._command("POST", "/execute/async", { script, args });
  }

  /** GET /session/:id/window/handles */
  getWindowHandles() {
    return this._command("GET", "/window/handles");
  }

  /** POST /session/:id/window — switch the session's target browsing context. */
  switchToWindow(handle) {
    return this._command("POST", "/window", { handle });
  }

  /** DELETE /session/:id — best-effort; never throws (the caller is about
   * to tear down the driver process regardless). */
  async delete() {
    try {
      await fetch(`${this.baseUrl}/session/${this.sessionId}`, {
        method: "DELETE",
        signal: AbortSignal.timeout(5000),
      });
    } catch {
      // Best-effort teardown.
    }
  }
}
