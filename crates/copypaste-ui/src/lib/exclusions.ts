// Windows exclusion rules mirror `exclusion_key` in
// `crates/copypaste-daemon/src/clipboard/windows_attribution.rs`.
// Keep the two together: a canonical form this file produces and that one
// does not recognise is an exclusion that reads as saved and never fires
// (DMY-158).
//
// macOS/Android use reverse-DNS bundle ids. Windows uses the process image
// name from Task Manager — `chrome.exe`, `chrome`, or a pasted path all
// name one process.

import pathBrowserify from "path-browserify-win32";

/** The package's own `index.d.ts` declares itself in terms of itself, so its
 *  module type is `any`. Node's signature is the API it implements. */
const win32 = (pathBrowserify as { win32: typeof import("node:path").win32 }).win32;

const BUNDLE_ID = /^[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)+$/;

/**
 * The last path segment, by Windows' separators rather than the host's.
 *
 * `path-browserify-win32` owns the parsing, as `typed-path` does in the daemon,
 * so both sides read a `C:` drive prefix as a prefix rather than a separator.
 * What is left is the same input cleanup the daemon keeps: the shell wraps a
 * path pasted from Explorer's address bar in quotes, and an entry ending in a
 * separator names a directory rather than the image of any process.
 */
function imageName(entry: string): string {
  const trimmed = entry.trim().replace(/^"+|"+$/g, "").trim();
  if (/[\\/]$/.test(trimmed)) return "";
  return win32.basename(trimmed).trim();
}

/**
 * The comparison key: two entries with the same key exclude the same process.
 *
 * `null` is an entry that names no application at all — the empty string, a
 * bare `.exe`, or on the other platforms anything that is not a bundle id.
 */
export function exclusionKey(entry: string, windows: boolean): string | null {
  if (!windows) {
    const trimmed = entry.trim();
    return BUNDLE_ID.test(trimmed) ? trimmed : null;
  }
  const name = imageName(entry);
  const stem = /\.exe$/i.test(name) ? name.slice(0, -4) : name;
  return stem === "" ? null : stem.toLowerCase();
}

/**
 * What gets stored and shown for an entry the user typed.
 *
 * On Windows that is always `<name>.exe`, the spelling the item itself carries
 * and the one Task Manager shows — never the bare stem, which would leave the
 * list disagreeing with every row it is about.
 */
export function canonicalExclusion(entry: string, windows: boolean): string | null {
  const key = exclusionKey(entry, windows);
  if (key === null) return null;
  return windows ? `${key}.exe` : key;
}

/** Whether `entry` already names one of `ids`, in whichever spelling. */
export function findExclusion(
  ids: readonly string[],
  entry: string,
  windows: boolean,
): string | undefined {
  const key = exclusionKey(entry, windows);
  if (key === null) return undefined;
  return ids.find((id) => exclusionKey(id, windows) === key);
}
