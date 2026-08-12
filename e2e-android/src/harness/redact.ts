/**
 * Values a run seeded on purpose, kept out of anything it publishes.
 *
 * Its own module so the guard that proves it can run without a device: every
 * other harness file reaches `adb.ts`, which shells out at import time.
 */
const redactions = new Set<string>();

export function redactFromEvidence(secret: string): void {
  if (secret) redactions.add(secret);
}

/** The property this upholds: a fixture seeded to prove masking is not what
 *  gets published when the assertion about masking it fails. */
export function redactFixtures(text: string): string {
  let out = text;
  for (const secret of redactions) out = out.split(secret).join("[redacted fixture]");
  return out;
}
