/**
 * What the UI leg records about the WebView it attached to.
 *
 * Not the run's nonce: nine digits publishes `AKIAHARNESS<nonce>` and, through
 * `leaks.android.test.ts`'s `nonce + 1`, the second credential; nine digits is
 * brute-forced, so no digest is safe either. Tests take it from `project.provide`.
 *
 * Device-free so `harness-guard.android.test.ts` can compose the whole uploaded
 * directory from it; every other harness module reaches `adb.ts`, which shells
 * out at import time.
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
