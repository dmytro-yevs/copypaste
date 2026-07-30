/**
 * The rules the catalogue makes checkable, now that the strings are in one
 * place rather than in forty components.
 *
 * INV-12 / AT-24: the daemon socket lives under the home directory, so its path
 * spells out the local username. `lib/errors.ts` keeps raw text out; this keeps
 * *authored* text from reintroducing the same leak by hand.
 */
import { describe, expect, it } from "vitest";

import { en } from "@/i18n/en";

type Entry = readonly [key: string, value: string];

function entriesOf(node: unknown, path: readonly string[] = []): Entry[] {
  if (typeof node === "string") return [[path.join("."), node]];
  if (typeof node !== "object" || node === null) return [];
  return Object.entries(node).flatMap(([key, child]) =>
    entriesOf(child, [...path, key]),
  );
}

const ALL = entriesOf(en);

const PLACEHOLDER = /\{\{\s*([A-Za-z0-9_]+)/g;

function placeholdersOf(value: string): string[] {
  return [...value.matchAll(PLACEHOLDER)].map(([, name]) => name!.toLowerCase());
}

describe("the catalogue", () => {
  it("has entries, and none of them is empty", () => {
    expect(ALL.length).toBeGreaterThan(200);
    for (const [key, value] of ALL) {
      expect(value.trim(), key).not.toBe("");
    }
  });

  /** i18next resolves `_one`/`_other` through `Intl.PluralRules`; a half-pair
   *  renders the key name at a count nobody tested. */
  it("pairs every plural form", () => {
    const forms = new Map<string, Set<string>>();
    for (const [key] of ALL) {
      const match = /^(.*)_(one|other|zero|two|few|many)$/.exec(key);
      if (!match) continue;
      const bucket = forms.get(match[1]!) ?? new Set<string>();
      bucket.add(match[2]!);
      forms.set(match[1]!, bucket);
    }
    expect(forms.size).toBeGreaterThan(0);
    for (const [stem, present] of forms) {
      expect([...present].sort(), stem).toEqual(["one", "other"]);
    }
  });
});

describe("no user-facing string can carry a filesystem path (INV-12)", () => {
  const SHAPES: ReadonlyArray<readonly [string, RegExp]> = [
    ["an absolute unix path", /\/(?:Users|home|private|var|tmp|etc|opt|usr|Applications|Library)\//],
    ["a home-relative path", /(?:^|[\s"'(<])~\//],
    ["a socket file", /\.sock\b/],
    ["a windows path", /[A-Za-z]:\\/],
    ["a path variable", /\$HOME|\$XDG_|%APPDATA%|%USERPROFILE%/],
    ["a database file", /\.(?:db|sqlite3?)\b/],
  ];

  it.each(SHAPES)("contains no %s", (_name, shape) => {
    for (const [key, value] of ALL) {
      expect(value, key).not.toMatch(shape);
    }
  });

  /**
   * The other half: a message with no path in it today, but a `{{path}}` hole
   * for one. A caller filling that hole is exactly how the socket path reaches
   * the DOM, and it would pass every assertion above.
   */
  it("offers no interpolation a path could be poured into", () => {
    const FORBIDDEN = new Set([
      "path",
      "paths",
      "file",
      "filename",
      "filepath",
      "dir",
      "directory",
      "folder",
      "socket",
      "sock",
      "home",
      "cwd",
      "location",
      "db",
      "database",
      "url",
      "uri",
    ]);
    for (const [key, value] of ALL) {
      for (const name of placeholdersOf(value)) {
        expect(FORBIDDEN.has(name), `${key} interpolates {{${name}}}`).toBe(false);
      }
    }
  });
});

/**
 * A clipping's text must not be an argument to `t`. A template is a second
 * place the plaintext lives, outside everything the sensitive-content rule
 * guards — and `previewOf` output is the same string the row is masking.
 */
it("offers no interpolation a clipping could be poured into", () => {
  const FORBIDDEN = new Set([
    "content",
    "contents",
    "clip",
    "clipping",
    "item",
    "preview",
    "plaintext",
    "secret",
    "password",
    "token",
    "body",
    "text",
    "value",
  ]);
  for (const [key, value] of ALL) {
    for (const name of placeholdersOf(value)) {
      expect(FORBIDDEN.has(name), `${key} interpolates {{${name}}}`).toBe(false);
    }
  }
});
