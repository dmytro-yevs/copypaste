/**
 * What the UI leg records about the WebView it attached to.
 *
 * The run's nonce is deliberately not among it. Nine digits publishes
 * `AKIAHARNESS<nonce>` and, through `leaks.android.test.ts`'s `nonce + 1`,
 * the second credential too — and hashing nine digits is brute-forced in
 * milliseconds, so there is no safe representation of it to publish instead.
 * Tests receive it through `project.provide`, which is not an artifact.
 *
 * Device-free on purpose: `harness-guard.android.test.ts` composes the whole
 * uploaded directory from this, and every other harness module reaches
 * `adb.ts`, which shells out at import time.
 */
import path from "node:path";

import { writeRedacted } from "./redact.js";

export interface AttachedWebView {
  package: string;
  pid: number | string;
  socket: string;
  url: string;
  title: string;
  version: Record<string, string>;
}

export function writeAttachment(dir: string, attached: AttachedWebView): void {
  writeRedacted(path.join(dir, "attachment.json"), attached);
}
