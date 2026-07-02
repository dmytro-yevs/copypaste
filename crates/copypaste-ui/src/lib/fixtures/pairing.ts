// ---------------------------------------------------------------------------
// src/lib/fixtures/pairing.ts — typed SAS-pairing fixture factory.
//
// DEV-only. See index.ts for the import-boundary rule.
// ---------------------------------------------------------------------------

import type { PairSasStatus } from "../ipc";

/**
 * Typed factory for the SAS-pairing poll state ({@link PairSasStatus}), with
 * per-field override support (design.md Decision 7/G3). Defaults to the idle
 * state (no pairing in flight, no modal on load) — override `state` (and the
 * `sas`/peer-identity fields it implies) to build any other pairing stage.
 */
export function makePairStatus(over: Partial<PairSasStatus> = {}): PairSasStatus {
  return {
    state: "idle",
    ...over,
  };
}
