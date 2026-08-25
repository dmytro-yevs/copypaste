/**
 * Everything this harness publishes goes through here — `attachment.json` and
 * the failure captures alike. `harness-guard.android.test.ts` holds that claim
 * to the source rather than to this sentence: `global-setup.ts` used to write
 * its attachment directly, and the sentence was simply wrong for a while.
 *
 * Its own module so the guard that proves it can run without a device: every
 * other harness file reaches `adb.ts`, which shells out at import time.
 */
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const REDACTED = "[redacted fixture]";

type PublicationRedaction = Readonly<{
  schemaVersion: 1;
  source: "config/sensitive-rules.toml";
  patterns: readonly Readonly<{ rule: string; pattern: string }>[];
}>;

const policy = JSON.parse(
  readFileSync(
    new URL("../../../config/sensitive-publication-redaction.json", import.meta.url),
    "utf8",
  ),
) as PublicationRedaction;

if (
  policy.schemaVersion !== 1 ||
  policy.source !== "config/sensitive-rules.toml" ||
  !Array.isArray(policy.patterns) ||
  policy.patterns.length === 0 ||
  policy.patterns.some(({ rule, pattern }) => !rule || !pattern)
) {
  throw new Error("unsupported publication-redaction policy");
}

const SENSITIVE_PATTERNS = policy.patterns.map(({ pattern }) => new RegExp(pattern, "gu"));

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
  for (const pattern of SENSITIVE_PATTERNS) out = out.replace(pattern, REDACTED);
  return out;
}

/** The only way this harness writes a file a run publishes, enforced by the
 *  guard rather than asserted here. */
export function writeRedacted(file: string, value: unknown): void {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, redactFixtures(JSON.stringify(value, null, 2)));
}
