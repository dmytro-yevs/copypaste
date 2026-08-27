/**
 * Nothing here ever sees the plaintext of a sensitive item: the bridge sends
 * `content: null` for one (INV-10), so `previewOf` is only ever reached for
 * content that exists, and `kindOf` decides `secret` from the flag rather than
 * from the text.
 */
import type { Item } from "./ipc";

/* ------------------------------------------------------------------ age --- */

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const WEEK = 7 * DAY;

const RELATIVE = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
const ABSOLUTE = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  // The compact age is only a scanning aid. When a person asks for the
  // timestamp, show the locally-formatted time precisely enough to correlate
  // it with an event in another app or device.
  timeStyle: "medium",
});

/** Compact age for the row's meta line: "now", "4m", "2h", "3d", "6w". */
export function shortAge(createdAt: number, now: number = Date.now()): string {
  const delta = Math.max(0, now - createdAt);
  if (delta < MINUTE) return "now";
  if (delta < HOUR) return `${Math.floor(delta / MINUTE)}m`;
  if (delta < DAY) return `${Math.floor(delta / HOUR)}h`;
  if (delta < WEEK) return `${Math.floor(delta / DAY)}d`;
  return `${Math.floor(delta / WEEK)}w`;
}

/** Spoken age, for accessible names and the pointer tooltip. */
export function longAge(createdAt: number, now: number = Date.now()): string {
  const delta = Math.max(0, now - createdAt);
  if (delta < MINUTE) return "just now";
  if (delta < HOUR) return RELATIVE.format(-Math.floor(delta / MINUTE), "minute");
  if (delta < DAY) return RELATIVE.format(-Math.floor(delta / HOUR), "hour");
  if (delta < WEEK) return RELATIVE.format(-Math.floor(delta / DAY), "day");
  return RELATIVE.format(-Math.floor(delta / WEEK), "week");
}

export function absoluteTime(createdAt: number): string {
  return ABSOLUTE.format(createdAt);
}

/* -------------------------------------------------------------- preview --- */

/** Collapse the runs of blank lines a copied document tends to carry, so the
 *  reserved preview lines show that many lines of actual content. */
export function previewOf(content: string): string {
  return content.replace(/\s*\n\s*\n\s*/g, "\n").trim();
}

export function truncate(text: string, max: number): string {
  const flat = text.replace(/\s+/g, " ").trim();
  return flat.length <= max ? flat : `${flat.slice(0, max - 1)}…`;
}

/* ----------------------------------------------------------------- kind --- */

/** The kind set is closed and ends in `unknown`; each maps onto a `--c-*`
 *  token, and an unrecognised kind renders as `unknown` rather than blank. */
export type Kind =
  | "secret"
  | "image"
  | "file"
  | "url"
  | "mail"
  | "path"
  | "json"
  | "code"
  | "color"
  | "num"
  | "text"
  | "unknown";

const RULES: ReadonlyArray<readonly [Kind, RegExp]> = [
  ["url", /^(https?|ftp):\/\/\S+$/i],
  ["mail", /^[^\s@]+@[^\s@]+\.[^\s@]+$/],
  ["path", /^(file:\/\/|~|\/|\.{1,2}\/)[^\n]*$/i],
  ["color", /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$|^(rgba?|hsla?)\(/i],
  ["num", /^[-+]?\d[\d\s,._]*$/],
];

/**
 * The Rust-owned class chooses the base kind. Text can gain a display-only
 * decoration from its contents, but an unfamiliar binary type stays unknown.
 */
export function kindOf(item: Item): Kind {
  if (item.is_sensitive) return "secret";
  if (item.content_class === "image") return "image";
  if (item.content_class === "file") return "file";
  if (item.content_class === "other" || item.content === null)
    return "unknown";

  const trimmed = item.content.trim();
  if (!trimmed) return "text";

  for (const [kind, pattern] of RULES) {
    if (pattern.test(trimmed)) return kind;
  }

  if (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  ) {
    return "json";
  }
  if (/[;{}]\s*$/m.test(trimmed) && trimmed.includes("\n")) return "code";

  return "text";
}

/** Kinds whose content reads better in the mono face (manifest §3.1.5). */
export const MONO_KINDS: ReadonlySet<Kind> = new Set<Kind>([
  "code",
  "json",
  "num",
  "color",
  "path",
]);
