// ---------------------------------------------------------------------------
// src/lib/fixtures/time.ts — shared relative-time helpers for fixture data.
//
// DEV-only. See index.ts for the import-boundary rule.
//
// FIXTURE_NOW is computed once at module evaluation (an ES module is a
// singleton, so this happens exactly once per process) — mirrors the previous
// behaviour of the `NOW` constant that lived inline in mockIpc.ts before this
// module existed, so refactoring mockIpc.ts to import these helpers changes no
// observable fixture value (byte-identical timestamps for a given call).
// ---------------------------------------------------------------------------

export const FIXTURE_NOW = Date.now();

/** `n` minutes before {@link FIXTURE_NOW}, in epoch milliseconds. */
export const mins = (n: number): number => FIXTURE_NOW - n * 60_000;

/** `n` hours before {@link FIXTURE_NOW}, in epoch milliseconds. */
export const hours = (n: number): number => FIXTURE_NOW - n * 3_600_000;

/** `n` days before {@link FIXTURE_NOW}, in epoch milliseconds. */
export const days = (n: number): number => FIXTURE_NOW - n * 86_400_000;
