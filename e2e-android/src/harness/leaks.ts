/**
 * The two constraints from `e2e/src/harness/leaks.ts`, asserted against the
 * Android WebView instead of WebKitGTK.
 *
 * They are DOM-level, so they port unchanged in substance. What changes is the
 * path vocabulary: Android runs no daemon and so has no socket (ADR-0002), and
 * the string that would disclose the user there is the app's private directory
 * under `/data`.
 */
import { expect } from "vitest";

import {
  ANDROID_PATH_LEAK_PATTERNS,
  findFilesystemPathLeaks,
} from "../../../test-support/security/path-leaks.js";
import { PACKAGE } from "./adb.js";
import type { AndroidApp } from "./app.js";

const NAMING_ATTRIBUTES = [
  "aria-label",
  "aria-description",
  "aria-valuetext",
  "title",
  "alt",
  "placeholder",
  "value",
];

/** Rendered text plus the attributes that name things — `innerText` alone
 *  misses `title=` and `aria-label=`, which is where a screen that is "only
 *  showing a code" puts the thing it is not showing. */
export async function accessibleSurface(app: AndroidApp): Promise<string> {
  return app.withPage((page) =>
    page.evaluate((attributes: string[]) => {
      const parts: string[] = [document.body.innerText];
      for (const node of Array.from(document.querySelectorAll("*"))) {
        for (const attribute of attributes) {
          const value = node.getAttribute(attribute);
          if (value) parts.push(value);
        }
      }
      return parts.join("\n");
    }, NAMING_ATTRIBUTES),
  );
}

export async function outerHtml(app: AndroidApp): Promise<string> {
  return app.withPage((page) => page.evaluate(() => document.documentElement.outerHTML));
}

/** INV-12 / AGENTS.md rule 4. */
export function expectNoFilesystemPath(surface: string, ...forbidden: readonly string[]): void {
  expect(
    findFilesystemPathLeaks(surface, {
      forbidden: [
        ...forbidden,
        `/data/data/${PACKAGE}`,
        `/data/user/0/${PACKAGE}`,
      ],
      additions: ANDROID_PATH_LEAK_PATTERNS,
    }),
    "filesystem path leaked to the accessible surface",
  ).toEqual([]);
}

export function expectNoRawError(html: string): void {
  expect(html).not.toMatch(/Connection refused|ECONNREFUSED|ENOENT|No such file/i);
}

/** INV-10 / INV-13: absent from the document, not merely covered — a blur, a
 *  `display: none` or a masked font all leave it in `outerHTML`. */
export async function expectSecretAbsent(app: AndroidApp, secret: string): Promise<void> {
  expect(await outerHtml(app)).not.toContain(secret);

  const values = await app.withPage((page) =>
    page.evaluate(() =>
      Array.from(document.querySelectorAll("input,textarea"))
        .map((node) => (node as HTMLInputElement).value)
        .join("\n"),
    ),
  );
  expect(values).not.toContain(secret);
}
