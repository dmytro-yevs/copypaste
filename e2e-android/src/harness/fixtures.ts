/**
 * One nonce per run, because the store deduplicates: the same text shared twice
 * inside the dedup interval is one row, and a second run would then be
 * asserting against the first run's item.
 */
import { redactFromEvidence } from "./redact.js";

declare module "vitest" {
  interface ProvidedContext {
    nonce: string;
  }
}

/**
 * Minting a fixture registers it, because suites mint their own: `leaks` derives
 * a second nonce of its own, and a registry the caller has to remember is one a
 * caller eventually forgets — with a published credential as the cost.
 *
 * The tail is registered as well as the credential. Both fixtures carry it and
 * the credential is `AKIA` plus it, so publishing the tail republishes the
 * credential.
 */
function harnessTail(nonce: string): string {
  const tail = `HARNESS${nonce}`;
  redactFromEvidence(tail);
  return tail;
}

/** `AKIA[0-9A-Z]{16}`, the `aws_access_key` rule in
 *  `crates/copypaste-core/src/sensitive/rules_generated.rs`. Confidence 0.99,
 *  which is what makes the flag deterministic rather than a band judgement. */
export function secretFor(nonce: string): string {
  const secret = `AKIA${harnessTail(nonce)}`;
  redactFromEvidence(secret);
  return secret;
}

export function ordinaryFor(nonce: string): string {
  return `an ordinary clipping ${harnessTail(nonce)}`;
}

export function fixtureMarker(prefix: string, entropy = Date.now()): string {
  return `${prefix}-m${entropy.toString(36)}`;
}

/** Nine digits, so `secretFor` lands on exactly the 16 the rule wants. */
export function freshNonce(): string {
  return String(Date.now() % 1_000_000_000).padStart(9, "0");
}
