/**
 * The two constraints every screen in this app is under, in one place.
 *
 * They are asserted rather than assumed, and they are asserted against the
 * *document* rather than against the component that was supposed to enforce
 * them — a leak arrives through whichever screen forgot, so the check has to be
 * cheap enough for every file to run it.
 */
import { expect } from "vitest";

import {
  DESKTOP_PATH_LEAK_PATTERNS,
  findFilesystemPathLeaks,
} from "../../../test-support/security/path-leaks.js";
import type { Browser } from "./webview-guard.js";

/** Attributes that carry a name, a tooltip or a value to a user or an AT. */
const NAMING_ATTRIBUTES = [
  "aria-label",
  "aria-description",
  "aria-valuetext",
  "title",
  "alt",
  "placeholder",
  "value",
];

/**
 * Everything a user or a screen reader can reach: rendered text plus the
 * attributes that name things. `innerText` alone misses `title=` and
 * `aria-label=`, which is exactly where a "we only show a code" screen puts the
 * thing it is not showing.
 */
export async function accessibleSurface(browser: Browser): Promise<string> {
  return (await browser.execute(function (attributes: string[]) {
    const parts: string[] = [document.body.innerText];
    for (const node of Array.prototype.slice.call(document.querySelectorAll("*"))) {
      for (const attribute of attributes) {
        const value = (node as Element).getAttribute(attribute);
        if (value) parts.push(value);
      }
    }
    return parts.join("\n");
  }, NAMING_ATTRIBUTES)) as string;
}

/**
 * The whole document as markup.
 *
 * Used for "this string is absent", never for "no path is present": a dev build
 * loads its modules from the Vite server, so the markup legitimately carries
 * URL paths that the accessible surface does not.
 */
export async function outerHtml(browser: Browser): Promise<string> {
  return (await browser.execute(
    () => document.documentElement.outerHTML,
  )) as string;
}

/** INV-12 / AGENTS.md rule 4: user-facing errors may not disclose paths. */
export function expectNoFilesystemPath(
  surface: string,
  ...forbidden: readonly string[]
): void {
  expect(
    findFilesystemPathLeaks(surface, {
      forbidden,
      additions: DESKTOP_PATH_LEAK_PATTERNS,
    }),
    "filesystem path leaked to the accessible surface",
  ).toEqual([]);
}

/** No raw transport wording, which is where a path arrives from (INV-12). */
export function expectNoRawError(html: string): void {
  expect(html).not.toMatch(/Connection refused|ECONNREFUSED|ENOENT|No such file/i);
}

/**
 * INV-10 / INV-13: a secret must be absent from the document, not merely
 * covered — a blur, a `display: none` or a masked font leave it in `outerHTML`,
 * in the accessibility tree and in anything that copies the DOM.
 */
export async function expectSecretAbsent(
  browser: Browser,
  secret: string,
): Promise<void> {
  expect(await outerHtml(browser)).not.toContain(secret);

  // A field's live value is not in the markup.
  const values = (await browser.execute(() =>
    Array.prototype.map
      .call(
        document.querySelectorAll("input,textarea"),
        (node) => (node as HTMLInputElement).value,
      )
      .join("\n"),
  )) as string;
  expect(values).not.toContain(secret);
}
