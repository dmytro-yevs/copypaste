/**
 * Everything this harness publishes goes through here — `attachment.json` and
 * the failure captures alike. `harness-guard.android.test.ts` holds that claim
 * to the source rather than to this sentence: `global-setup.ts` used to write
 * its attachment directly, and the sentence was simply wrong for a while.
 *
 * Its own module so the guard that proves it can run without a device: every
 * other harness file reaches `adb.ts`, which shells out at import time.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

const REDACTED = "[redacted fixture]";

const redactions = new Set<string>();

export function redactFromEvidence(secret: string): void {
  if (secret) redactions.add(secret);
}

/** Explicit run fixtures are kept out of published diagnostics. */
function redactFixtures(text: string): string {
  let out = text;
  // Longest first prevents a short fixture marker from partially replacing a
  // longer marker that contains it.
  for (const secret of [...redactions].sort((a, b) => b.length - a.length)) {
    out = out.split(secret).join(REDACTED);
  }
  return out;
}

/** The only way this harness writes a file a run publishes, enforced by the
 *  guard rather than asserted here. */
export function writeRedacted(file: string, value: unknown): void {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, redactFixtures(JSON.stringify(value, null, 2)));
}

/** The screenshot caller removes every text and media surface before capture. */
export function writeSafeScreenshot(file: string, base64Png: string): void {
  const png = Buffer.from(base64Png, "base64");
  if (
    png.length < 8 ||
    !png.subarray(0, 8).equals(Buffer.from("89504e470d0a1a0a", "hex"))
  ) {
    throw new Error("setup evidence screenshot was not a PNG");
  }
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, png);
}
