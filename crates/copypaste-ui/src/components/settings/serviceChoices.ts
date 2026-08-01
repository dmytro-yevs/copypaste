/**
 * The values each numeric service setting offers.
 *
 * # Why a fixed set and not a number field
 *
 * `copypaste_ipc::config` keeps the bounds for every field — its own header
 * says the Settings UI is one of the three parties that needs them — but the
 * consts are private to that crate, so nothing here can read them. Restating
 * `100..=60_000` in TypeScript is exactly the drift the module was written to
 * end (v1 had the limits in one crate, the clamp in another, and the UI's own
 * guesses in a third).
 *
 * A closed set of choices needs no bounds: every value below is well inside
 * them, so an out-of-range write is unreachable from this screen rather than
 * caught by a copy of a rule. It also reads better — these are settings where a
 * user wants "a minute", not the ability to type 47.
 *
 * The remaining gap is deliberate and recorded: until `copypaste-ipc` publishes
 * its bounds, a value the service would accept and this list does not offer is
 * reachable only from the CLI. `valuesWith` keeps such a value visible rather
 * than silently snapping the control to a neighbour.
 */

/** The leaf keys under `settings.service.units`. A union rather than `string`
 *  so `t` still type-checks the interpolated key. */
export type Unit =
  | "off"
  | "never"
  | "raw"
  | "ms"
  | "seconds"
  | "minutes"
  | "hours"
  | "days"
  | "items"
  | "kilobytes"
  | "megabytes"
  | "gigabytes";

export interface Choice {
  readonly value: number;
  readonly unit: Unit;
  /** The number `{{count}}` renders — not always `value` (500 ms is
   *  `{{count}} ms` with 500; 3600 s is `{{count}} hour` with 1). */
  readonly count: number;
}

const ms = (value: number): Choice => ({ value, unit: "ms", count: value });
const seconds = (value: number): Choice => ({ value, unit: "seconds", count: value });
const minutes = (value: number): Choice => ({
  value,
  unit: "minutes",
  count: value / 60,
});
const hours = (value: number): Choice => ({ value, unit: "hours", count: value / 3600 });
const days = (value: number): Choice => ({ value, unit: "days", count: value });
const items = (value: number): Choice => ({ value, unit: "items", count: value });
const megabytes = (mb: number): Choice => ({
  value: mb * 1_048_576,
  unit: "megabytes",
  count: mb,
});
const gigabytes = (gb: number): Choice => ({
  value: gb * 1_073_741_824,
  unit: "gigabytes",
  count: gb,
});
const kilobytes = (kb: number): Choice => ({
  value: kb * 1_024,
  unit: "kilobytes",
  count: kb,
});

/** `0` is a disabled sentinel on three of these fields, never "do it
 *  immediately" — the distinction is the whole of `CopyPaste-8ebg.1`. */
const OFF: Choice = { value: 0, unit: "off", count: 0 };
const NEVER: Choice = { value: 0, unit: "never", count: 0 };

export const POLL_INTERVAL_MS: readonly Choice[] = [
  ms(250),
  ms(500),
  ms(1000),
  ms(2000),
  ms(5000),
];

export const HISTORY_LIMIT: readonly Choice[] = [
  items(100),
  items(500),
  items(1000),
  items(5000),
  items(10000),
  items(50000),
];

/** Starts at the service's 50 MiB minimum and includes the 10 GiB default. */
export const STORAGE_QUOTA_BYTES: readonly Choice[] = [
  megabytes(50),
  gigabytes(1),
  gigabytes(5),
  gigabytes(10),
  gigabytes(50),
];

export const RETENTION_DAYS: readonly Choice[] = [
  NEVER,
  days(7),
  days(30),
  days(90),
  days(365),
];

export const DEDUP_WINDOW_SECS: readonly Choice[] = [
  OFF,
  seconds(10),
  seconds(30),
  minutes(60),
  minutes(300),
];

/** Stops at 4 MiB because `copypaste_ipc::MAX_CONTENT_BYTES` is the ceiling the
 *  daemon accepts — a larger choice here is a control that only ever errors. */
export const MAX_ITEM_BYTES: readonly Choice[] = [
  kilobytes(256),
  megabytes(1),
  megabytes(4),
];

/**
 * `30` first, because it is the value manifest 01 §4 and manifest 07 §6.2 both
 * give and the one `copypaste_core::sensitive::DEFAULT_SENSITIVE_TTL` still
 * holds. It is a suggestion here and not a default: the field ships at `0`, and
 * this control is the interface whose absence is why.
 */
export const SENSITIVE_TTL_SECS: readonly Choice[] = [
  OFF,
  seconds(30),
  minutes(300),
  hours(3600),
  hours(21600),
];

/**
 * The offered values, plus whatever the service is actually on.
 *
 * Without this a value set from the CLI would have no option to match, the
 * control would render as though it were on the first choice, and the next
 * touch of any *other* setting would leave the user believing this one had said
 * what it showed. An unlisted value appears as itself instead.
 */
export function valuesWith(choices: readonly Choice[], current: number): readonly Choice[] {
  if (choices.some((choice) => choice.value === current)) return choices;
  const unlisted: Choice = { value: current, unit: "raw", count: current };
  return [...choices, unlisted].sort((a, b) => a.value - b.value);
}
