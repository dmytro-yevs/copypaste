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

/**
 * `AKIA[0-9A-Z]{16}` — the `aws_access_key` rule in
 * `crates/copypaste-core/src/sensitive/rules_generated.rs` that the seeded
 * fixtures are shaped to trip. Registration covers what this harness minted;
 * the shape covers what it did not, so publication is not a function of
 * somebody remembering to register.
 */
const SENSITIVE_SHAPE = /AKIA[0-9A-Z]{16}/g;

const redactions = new Set<string>();

export function redactFromEvidence(secret: string): void {
  if (secret) redactions.add(secret);
}

/** The property this upholds: a fixture seeded to prove masking is not what
 *  gets published when the assertion about masking it fails. */
function redactFixtures(text: string): string {
  let out = text;
  // Longest first. `HARNESS<nonce>` is a suffix of `AKIAHARNESS<nonce>`, and
  // replacing the short one first would leave `AKIA` welded to the placeholder.
  for (const secret of [...redactions].sort((a, b) => b.length - a.length)) {
    out = out.split(secret).join(REDACTED);
  }
  return out.replace(SENSITIVE_SHAPE, REDACTED);
}

/** The only way this harness writes a file a run publishes, enforced by the
 *  guard rather than asserted here. */
export function writeRedacted(file: string, value: unknown): void {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, redactFixtures(JSON.stringify(value, null, 2)));
}
