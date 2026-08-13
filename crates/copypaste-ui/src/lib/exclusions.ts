// Windows exclusion rules mirror `exclusion_key` in
// `crates/copypaste-daemon/src/clipboard/windows_attribution.rs`.
// Keep the two together: a canonical form this file produces and that one
// does not recognise is an exclusion that reads as saved and never fires
// (DMY-158).
//
// macOS/Android use reverse-DNS bundle ids. Windows uses the process image
// name from Task Manager — `chrome.exe`, `chrome`, or a pasted path all
// name one process.

const BUNDLE_ID = /^[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)+$/;

/** The last path segment, by Windows' separators rather than the host's. */
function imageName(entry: string): string {
  const trimmed = entry.trim().replace(/^"+|"+$/g, "");
  const segments = trimmed.split(/[\\/:]/);
  return (segments[segments.length - 1] ?? "").trim();
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
