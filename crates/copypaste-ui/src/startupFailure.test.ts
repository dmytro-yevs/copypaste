import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { atStartup, reportStartupFailure } from "@/startupFailure";

function root(): HTMLElement {
  const found = document.getElementById("root");
  if (!found) throw new Error("the test did not install #root");
  return found;
}

/** The whole point of the boundary: a startup that failed must be readable. */
function notice(): HTMLElement {
  const found = root().querySelector<HTMLElement>('[role="alert"]');
  if (!found) throw new Error("no startup failure was rendered");
  return found;
}

beforeEach(() => {
  document.body.innerHTML = '<div id="root"><p>a half-drawn screen</p></div>';
  vi.spyOn(console, "error").mockImplementation(() => {});
  vi.spyOn(console, "warn").mockImplementation(() => {});
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllEnvs();
  vi.resetModules();
});

describe("the startup boundary", () => {
  test("passes a stage through and keeps its value", async () => {
    await expect(atStartup("screens", () => Promise.resolve(7))).resolves.toBe(7);
  });

  test("reports the stage that failed, not the error it failed with", async () => {
    const asset = "http://tauri.localhost/assets/legacyPolyfills-legacy-Duwqjisk.js";
    await expect(
      atStartup("polyfills", () => Promise.reject(new Error(`Failed to fetch ${asset}`))),
    ).rejects.toMatchObject({ startupStage: "polyfills" });
  });

  test("renders one visible announced failure over whatever was drawn", () => {
    reportStartupFailure(root(), { startupStage: "screens" });

    expect(root().textContent).not.toContain("a half-drawn screen");
    expect(notice().dataset.startupFailure).toBe("screens");
    expect(notice().querySelector("h1")?.textContent).toBe("CopyPaste could not start");
    expect(notice().textContent).toContain("The app's screens did not load.");
    // Announced without a reload: `role="alert"` plus focus, since a WebView
    // that never rendered has nothing else for a screen reader to land on.
    expect(notice().getAttribute("tabindex")).toBe("-1");
    expect(document.activeElement).toBe(notice());
  });

  test("names a stage it does not know rather than rendering nothing", () => {
    reportStartupFailure(root(), new Error("boom"));

    expect(notice().dataset.startupFailure).toBe("unknown");
    expect(notice().textContent).toContain("The app did not finish starting.");
  });

  test("a name off Object.prototype is not a stage", () => {
    reportStartupFailure(root(), { startupStage: "toString" });

    expect(notice().dataset.startupFailure).toBe("unknown");
    expect(notice().textContent).toContain("The app did not finish starting.");
  });

  // Rule 4: a failed import's message is a URL, and neither the notice nor the
  // log may repeat it.
  test("keeps the failed asset out of the notice and the log", async () => {
    const asset = "file:///Users/someone/CopyPaste/dist/assets/index-DJ.js";
    const failure = await atStartup("screens", () => {
      throw new Error(`Failed to fetch dynamically imported module: ${asset}`);
    }).catch((thrown: unknown) => thrown);
    reportStartupFailure(root(), failure);

    expect(notice().textContent).not.toContain("/");
    expect(console.error).toHaveBeenCalledExactlyOnceWith("[copypaste] startup failed: screens");
  });
});

/**
 * The wiring, not the boundary: `main.tsx` owns which stage each import belongs
 * to, and a rejection that reaches no `catch` is the blank `#root` this exists
 * to end.
 */
describe("a startup that never reaches the first render", () => {
  async function bootWith(module: string, reason: string): Promise<void> {
    vi.resetModules();
    vi.doMock(module, () => {
      throw new Error(reason);
    });
    await import("@/main");
    // `vi.resetModules()` means the whole entry graph is re-evaluated per boot,
    // so this waits on real work rather than on a poll; the 1000ms default is a
    // budget the graph has outgrown.
    await vi.waitFor(() => notice(), { timeout: 5000 });
  }

  test("shows the polyfill stage when the legacy polyfills cannot load", async () => {
    vi.stubEnv("LEGACY", "true");
    await bootWith("@/legacyPolyfills", "Failed to fetch /assets/legacyPolyfills-legacy.js");

    expect(notice().dataset.startupFailure).toBe("polyfills");
    expect(notice().textContent).toContain("compatibility layer");
  });

  test("shows the screen stage when an app chunk cannot load", async () => {
    await bootWith("@/app/App", "Failed to fetch dynamically imported module: /assets/App.js");

    expect(notice().dataset.startupFailure).toBe("screens");
    expect(notice().textContent).toContain("screens did not load");
  });
});

/**
 * The render boundary: a component that throws during React 19's asynchronous
 * first render must still reach the application-owned notice, not a blank root.
 */
describe("a component that throws during the first render", () => {
  test("shows the render stage and redacts the thrown error", async () => {
    vi.resetModules();
    vi.doMock("@/app/App", () => ({
      default: function BrokenApp(): never {
        throw new Error("render exploded: /Users/someone/CopyPaste/src/App.tsx");
      },
    }));
    await import("@/main");
    // `vi.resetModules()` means the whole entry graph is re-evaluated per boot,
    // so this waits on real work rather than on a poll; the 1000ms default is a
    // budget the graph has outgrown.
    await vi.waitFor(() => notice(), { timeout: 5000 });

    expect(notice().dataset.startupFailure).toBe("render");
    expect(notice().textContent).toContain("could not draw its first screen");
    expect(notice().textContent).not.toContain("/");
    expect(notice().getAttribute("tabindex")).toBe("-1");
    expect(document.activeElement).toBe(notice());
    expect(console.error).toHaveBeenCalledWith("[copypaste] startup failed: render");
  });
});
