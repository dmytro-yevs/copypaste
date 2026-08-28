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
import {
  writeSetupFailureEvidence,
  type SetupEvidenceProbe,
} from "./setup-evidence.js";
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
      (ready: string, nav: string, list: string, row: string, search: string) => {
        const tabs = Array.from(document.querySelectorAll(`${nav} button`), (node) => ({
          label: node.textContent?.trim() ?? "",
          current: node.getAttribute("aria-current") === "page",
          disabled: (node as HTMLButtonElement).disabled,
        }));
        return {
          url: `${location.origin}${location.pathname}`,
          route: tabs.find((tab) => tab.current)?.label ?? null,
          navigationReady: document.querySelectorAll(ready).length,
          tabs,
          searchPresent: document.querySelector(search) !== null,
          searchPopulated: Boolean(
            (document.querySelector(search) as HTMLInputElement | null)?.value,
          ),
          listPresent: document.querySelector(list) !== null,
          renderedRows: document.querySelectorAll(row).length,
        };
      },
      NAVIGATION_READY,
      NAV,
      HISTORY_LIST,
      ROW,
      SEARCH,
    ),
  );
}

function logSummary(logcat: string): unknown {
  const lines = logcat.split("\n").slice(-200);
  const count = (pattern: RegExp) =>
    lines.filter((line) => pattern.test(line)).length;
  return {
    linesSampled: lines.length,
    tauriConsole: count(/Tauri\/Console/),
    androidRuntime: count(/AndroidRuntime/),
    fatalException: count(/FATAL EXCEPTION/),
    fatalSignal: count(/Fatal signal/),
    renderProcess: count(/Render process/),
  };
}

function attachedProbe(app: AndroidApp): SetupEvidenceProbe {
  return {
    shell: () => shellState(app),
    hierarchy: () =>
      app.withPage((page) =>
        page.evaluate(() => {
          const pending: Array<{ node: Element; depth: number }> = document.body
            ? [{ node: document.body, depth: 0 }]
            : [];
          const nodes: unknown[] = [];
          while (pending.length && nodes.length < 300) {
            const current = pending.shift();
            if (!current) break;
            const { node, depth } = current;
            const box = node.getBoundingClientRect();
            const style = getComputedStyle(node);
            nodes.push({
              depth,
              tag: node.tagName.toLowerCase(),
              role: node.getAttribute("role"),
              current: node.getAttribute("aria-current"),
              disabled: node.getAttribute("aria-disabled"),
              expanded: node.getAttribute("aria-expanded"),
              hidden: node.getAttribute("aria-hidden"),
              visible:
                style.display !== "none" &&
                style.visibility !== "hidden" &&
                box.width > 0 &&
                box.height > 0,
              box: {
                x: Math.round(box.x),
                y: Math.round(box.y),
                width: Math.round(box.width),
                height: Math.round(box.height),
              },
              children: node.children.length,
            });
            if (depth < 12) {
              pending.push(
                ...Array.from(node.children, (child) => ({
                  node: child,
                  depth: depth + 1,
                })),
              );
            }
          }
          return { nodes, truncated: pending.length > 0 };
        }),
      ),
    screenshot: () =>
      app.withPage(async (page) => {
        const styleId = "copypaste-evidence-redaction";
        await page.evaluate((id) => {
          document.getElementById(id)?.remove();
          const style = document.createElement("style");
          style.id = id;
          style.textContent = [
            "*,*::before,*::after{color:transparent!important;-webkit-text-fill-color:transparent!important;text-shadow:none!important;background-image:none!important}",
            "input,textarea{-webkit-text-security:disc!important}",
            "img,picture,video,canvas,svg,iframe,object,embed{visibility:hidden!important}",
          ].join("");
          document.head.append(style);
        }, styleId);
        try {
          const image = await page.screenshot({
            encoding: "base64",
            type: "png",
            captureBeyondViewport: false,
          });
          if (typeof image !== "string") {
            throw new Error("screenshot encoding was not base64");
          }
          return image;
        } finally {
          await page.evaluate((id) => document.getElementById(id)?.remove(), styleId);
        }
      }),
    logs: async () => {
      const { logcatDump } = await import("./adb.js");
      return logSummary(await logcatDump(200));
    },
  };
}

export async function captureSetupFailure(
  suite: string,
  stage = "before-all",
): Promise<void> {
  if (!attached) {
    throw new Error("required setup evidence app attachment was unavailable");
  }
  await writeSetupFailureEvidence(suite, stage, attachedProbe(attached), OUT);
}

/**
 * The UI leg publishes nothing when it never attaches, so run 31671766432's
 * API 29 legs were diagnosable only from a different leg's logcat.
 */
export function writeAttachFailure(record: unknown): void {
  try {
    writeRedacted(path.join(OUT, "attach-failure.json"), record);
  } catch {
    /* the attachment failure is the one worth reporting */
  }
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
