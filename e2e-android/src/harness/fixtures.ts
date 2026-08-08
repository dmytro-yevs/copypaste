/**
 * One nonce per run, because the store deduplicates: the same text shared twice
 * inside the dedup interval is one row, and a second run would then be
 * asserting against the first run's item.
 */
declare module "vitest" {
  interface ProvidedContext {
    nonce: string;
  }
}

/** `AKIA[0-9A-Z]{16}`, the `aws_access_key` rule in
 *  `crates/copypaste-core/src/sensitive/rules_generated.rs`. Confidence 0.99,
 *  which is what makes the flag deterministic rather than a band judgement. */
export function secretFor(nonce: string): string {
  return `AKIAHARNESS${nonce}`;
}

export function ordinaryFor(nonce: string): string {
  return `an ordinary clipping HARNESS${nonce}`;
}

/** Nine digits, so `secretFor` lands on exactly the 16 the rule wants. */
export function freshNonce(): string {
  return String(Date.now() % 1_000_000_000).padStart(9, "0");
}
