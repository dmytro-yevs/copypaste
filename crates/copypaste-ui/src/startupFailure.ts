/**
 * What the app says when it never started. Nothing here may import an app
 * module, a polyfill or React: this is the code that runs when one of those
 * failed to load, so it cannot depend on anything whose failure it reports.
 * That is also why the copy is English rather than i18n's, and why the DOM
 * calls stay within Chromium 53 — `append` and `replaceChildren` are later.
 *
 * The diagnostic is a stage and nothing else. A failed dynamic import's message
 * carries the asset URL it fetched, and rule 4 keeps paths out of what users
 * and logs are shown, so the original error is dropped rather than reported.
 */

export type StartupStage =
  | "polyfills"
  | "platform"
  | "platform-data"
  | "screens"
  | "render";

const WHAT: Record<StartupStage, string> = {
  polyfills: "This device needs a compatibility layer that did not load.",
  platform: "The app could not load this device's platform support.",
  "platform-data": "The app could not read this device's platform information.",
  screens: "The app's screens did not load.",
  render: "The app loaded but could not draw its first screen.",
};

const STAGE_KEY = "startupStage";

function startupFailure(stage: StartupStage): Error {
  const failure = new Error(`startup failed at ${stage}`);
  Object.defineProperty(failure, STAGE_KEY, { value: stage, enumerable: true });
  return failure;
}

function stageOf(failure: unknown): StartupStage | undefined {
  const stage =
    typeof failure === "object" && failure !== null
      ? (failure as Record<string, unknown>)[STAGE_KEY]
      : undefined;
  // Own property, not `in`: `"toString" in WHAT` is true, and the notice would
  // have stringified a function into its own text.
  return typeof stage === "string" && Object.prototype.hasOwnProperty.call(WHAT, stage)
    ? (stage as StartupStage)
    : undefined;
}

/** Runs one startup stage under this boundary; any failure becomes its stage. */
export async function atStartup<T>(stage: StartupStage, run: () => T | Promise<T>): Promise<T> {
  try {
    return await run();
  } catch {
    throw startupFailure(stage);
  }
}

/**
 * Empties `root` and puts one announced, visible failure in it. Emptied first
 * so a half-rendered screen cannot sit above the notice, and the container is
 * attached before its text so `role="alert"` has something to announce.
 */
export function reportStartupFailure(root: Element, failure: unknown): void {
  const stage = stageOf(failure);
  console.error(`[copypaste] startup failed: ${stage ?? "unknown"}`);

  const notice = document.createElement("div");
  notice.setAttribute("role", "alert");
  notice.setAttribute("tabindex", "-1");
  notice.setAttribute("data-startup-failure", stage ?? "unknown");
  notice.style.cssText =
    "margin:0;padding:1.25rem;font-family:system-ui,sans-serif;font-size:1rem;" +
    "line-height:1.5;color:#111111;background:#ffffff";

  root.textContent = "";
  root.appendChild(notice);

  const title = document.createElement("h1");
  title.style.cssText = "margin:0 0 0.5rem;font-size:1.125rem";
  title.textContent = "CopyPaste could not start";
  notice.appendChild(title);

  const detail = document.createElement("p");
  detail.style.cssText = "margin:0";
  detail.textContent =
    `${stage === undefined ? "The app did not finish starting." : WHAT[stage]} ` +
    "Close CopyPaste and open it again.";
  notice.appendChild(detail);

  if (typeof (notice as HTMLElement).focus === "function") notice.focus();
}
