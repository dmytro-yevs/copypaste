/**
 * What the WebView was showing when an assertion failed.
 *
 * The UI leg publishes `attachment.json` and nothing else, so run 31634096676's
 * two failures — a row window holding no long clipping, a prepend that never
 * moved the scroll offset — are not diagnosable from the artifact at all, while
 * the storage leg beside it publishes a screen per step.
 *
 * Row text goes into a published artifact, so it passes through `redact.ts`.
 */
import path from "node:path";

import { listSnapshot } from "./list.js";
import { writeRedacted } from "./redact.js";
import { HISTORY_LIST, NAV, NAVIGATION_READY, ROW, SEARCH } from "./ui.js";
import type { AndroidApp } from "./app.js";

const OUT = process.env.HARNESS_OUT ?? "artifacts/android-ui";

let attached: AndroidApp | null = null;

export function rememberAttachedApp(app: AndroidApp): void {
  attached = app;
}

/** Filesystem-safe and stable, so a rerun overwrites its own file rather than
 *  accumulating one per attempt. */
function slug(name: string): string {
  return name.replace(/[^a-zA-Z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 120) || "failure";
}

async function shellState(app: AndroidApp): Promise<unknown> {
  return app.withPage((page) =>
    page.evaluate(
      (ready: string, nav: string, list: string, row: string, search: string) => ({
        url: location.href,
        navigationReady: document.querySelectorAll(ready).length,
        tabs: Array.from(document.querySelectorAll(`${nav} button`), (node) => ({
          label: node.textContent?.trim() ?? "",
          current: node.getAttribute("aria-current") === "page",
          disabled: (node as HTMLButtonElement).disabled,
        })),
        search: (document.querySelector(search) as HTMLInputElement | null)?.value ?? null,
        listPresent: document.querySelector(list) !== null,
        renderedRows: document.querySelectorAll(row).length,
      }),
      NAVIGATION_READY,
      NAV,
      HISTORY_LIST,
      ROW,
      SEARCH,
    ),
  );
}

/**
 * Best effort by contract: this runs while a test is already failing, and an
 * error raised here would replace the assertion's message with this file's.
 */
export async function captureFailure(name: string): Promise<void> {
  const app = attached;
  if (!app) return;
  const evidence: Record<string, unknown> = { test: name };
  try {
    evidence.shell = await shellState(app);
  } catch (error) {
    evidence.shell = { unreadable: String(error) };
  }
  try {
    evidence.list = await listSnapshot(app);
  } catch (error) {
    evidence.list = { unreadable: String(error) };
  }
  try {
    writeRedacted(path.join(OUT, "failures", `${slug(name)}.json`), evidence);
  } catch {
    /* the assertion's own failure is the one worth reporting */
  }
}
