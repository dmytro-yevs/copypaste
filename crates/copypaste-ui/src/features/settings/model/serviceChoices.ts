/**
 * `copypaste_ipc::config` owns the numeric bounds; `rustParity.test.ts` pins
 * the repeated endpoints below to it. Closed choices make out-of-range writes
 * unreachable from this screen. A value accepted through the CLI may still be
 * absent here, so `valuesWith` keeps it visible instead of snapping the control
 * to a neighbouring choice.
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
const megabyteValue = (mb: number): Choice => ({
  value: mb,
  unit: "megabytes",
  count: mb,
});

/** `0` is a disabled sentinel on three of these fields, never "do it
 *  immediately" — the distinction is the whole of `CopyPaste-8ebg.1`. */
const OFF: Choice = { value: 0, unit: "off", count: 0 };
const NEVER: Choice = { value: 0, unit: "never", count: 0 };

export const POLL_INTERVAL_MS: readonly Choice[] = [
  ms(100),
  ms(250),
  ms(500),
  ms(1000),
  ms(2000),
  ms(5000),
];

export const POLL_INTERVAL_MIN_MS = 100;
export const POLL_INTERVAL_MAX_MS = 5_000;
export const MIN_TEXT_SIZE_BYTES = 64 * 1_024;
export const MIN_IMAGE_SIZE_BYTES = 1_048_576;
export const MIN_FILE_SIZE_BYTES = 1_048_576;
export const MAX_FILE_SIZE_BYTES_LIMIT = 4 * 1_048_576;
export const MIN_DECODED_IMAGE_MB = 1;

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

export const MAX_TEXT_SIZE_BYTES: readonly Choice[] = [
  kilobytes(64),
  megabytes(1),
  megabytes(2),
  megabytes(4),
];

export const MAX_IMAGE_SIZE_BYTES: readonly Choice[] = [
  megabytes(1),
  megabytes(2),
  megabytes(4),
];

export const MAX_FILE_SIZE_BYTES: readonly Choice[] = [
  megabytes(1),
  megabytes(2),
  megabytes(4),
];

export const MAX_DECODED_IMAGE_MB: readonly Choice[] = [
  megabyteValue(1),
  megabyteValue(25),
  megabyteValue(50),
  megabyteValue(100),
  megabyteValue(250),
];

/**
 * `30` first, matching `copypaste_core::sensitive::DEFAULT_SENSITIVE_TTL` and
 * the shipped `ConfigData` default. `0` remains the explicit off sentinel.
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
