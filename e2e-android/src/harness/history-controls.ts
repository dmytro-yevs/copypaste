import type { ListSnapshot } from "./list.js";

const MAX_DIAGNOSTIC_ID_LENGTH = 64;
const MAX_DIAGNOSTIC_IDS = 64;
const MAX_DIAGNOSTIC_JSON_LENGTH = 8_192;
const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const OPAQUE_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,63}$/;

export type AriaCheckedDiagnostic =
  | "true"
  | "false"
  | "mixed"
  | "missing"
  | "unknown";

export interface SafeDiagnosticIds {
  ids: string[];
  invalidCount: number;
  truncatedCount: number;
}

export interface HistoryFilterMenuDiagnostic {
  allKindsAriaChecked: AriaCheckedDiagnostic;
  linksAriaChecked: AriaCheckedDiagnostic;
  menuPresent: boolean;
  menuExpanded: boolean;
}

export interface HistoryFilterMenuObservation {
  allKindsAriaChecked: unknown;
  linksAriaChecked: unknown;
  menuPresent: unknown;
  menuExpanded: unknown;
}

export interface HistoryFilterDiagnostic {
  beforeIds: SafeDiagnosticIds;
  restoredIds: SafeDiagnosticIds;
  symmetricDifference: string[];
  matched: boolean;
  defaultTriggerCount: number | null;
  menu: HistoryFilterMenuDiagnostic;
  list: {
    totalSize: number | null;
    scrollTop: number | null;
    scrollHeight: number | null;
    clientHeight: number | null;
    frame: number | null;
    visibleItemCount: number | null;
  };
}

function boundedFiniteNumber(value: unknown): number | null {
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  const rounded = Math.round(value * 100) / 100;
  return Number.isFinite(rounded) ? rounded : null;
}

function boundedCount(value: unknown): number | null {
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    value < 0
  ) {
    return null;
  }
  return Math.min(value, MAX_DIAGNOSTIC_IDS);
}

function diagnosticId(value: unknown): string | null {
  if (typeof value !== "string" || value.length === 0) return null;
  if (value.length > MAX_DIAGNOSTIC_ID_LENGTH) return null;
  return UUID_PATTERN.test(value) || OPAQUE_ID_PATTERN.test(value)
    ? value
    : null;
}

export function safeDiagnosticIds(ids: readonly unknown[]): SafeDiagnosticIds {
  const safe: string[] = [];
  let invalidCount = 0;
  let truncatedCount = 0;

  for (const value of ids) {
    const id = diagnosticId(value);
    if (id === null) {
      if (typeof value === "string" && value.length > MAX_DIAGNOSTIC_ID_LENGTH) {
        truncatedCount += 1;
      } else {
        invalidCount += 1;
      }
      continue;
    }
    if (safe.length >= MAX_DIAGNOSTIC_IDS) {
      truncatedCount += 1;
      continue;
    }
    safe.push(id);
  }

  return { ids: safe, invalidCount, truncatedCount };
}

export function normalizeAriaChecked(value: unknown): AriaCheckedDiagnostic {
  if (
    value === "true" ||
    value === "false" ||
    value === "mixed" ||
    value === "missing" ||
    value === "unknown"
  ) {
    return value;
  }
  if (value === null || value === undefined || value === "") return "missing";
  return "unknown";
}

function symmetricDifference(
  expected: readonly string[],
  actual: readonly string[],
): string[] {
  const expectedSet = new Set(expected);
  const actualSet = new Set(actual);
  return sortedItemIds([
    ...expected.filter((id) => !actualSet.has(id)),
    ...actual.filter((id) => !expectedSet.has(id)),
  ]);
}

export function historyFilterDiagnostic(
  beforeIds: readonly string[],
  restoredIds: readonly string[],
  defaultTriggerCount: unknown,
  menu: HistoryFilterMenuObservation,
  list: ListSnapshot,
): HistoryFilterDiagnostic {
  const safeBeforeIds = safeDiagnosticIds(beforeIds);
  const safeRestoredIds = safeDiagnosticIds(restoredIds);
  return {
    beforeIds: safeBeforeIds,
    restoredIds: safeRestoredIds,
    symmetricDifference: symmetricDifference(
      safeBeforeIds.ids,
      safeRestoredIds.ids,
    ),
    matched: sameSortedItemIds(beforeIds, restoredIds),
    defaultTriggerCount: boundedCount(defaultTriggerCount),
    menu: {
      allKindsAriaChecked: normalizeAriaChecked(menu.allKindsAriaChecked),
      linksAriaChecked: normalizeAriaChecked(menu.linksAriaChecked),
      menuPresent: menu.menuPresent === true,
      menuExpanded: menu.menuExpanded === true,
    },
    list: {
      totalSize: boundedFiniteNumber(list.totalSize),
      scrollTop: boundedFiniteNumber(list.scrollTop),
      scrollHeight: boundedFiniteNumber(list.scrollHeight),
      clientHeight: boundedFiniteNumber(list.clientHeight),
      frame: boundedCount(list.frame),
      visibleItemCount: boundedCount(
        list.rows.filter((row) => row.id !== "").length,
      ),
    },
  };
}

export function safeJSON(value: unknown): string {
  try {
    const encoded = JSON.stringify(value);
    if (typeof encoded === "string" && encoded.length <= MAX_DIAGNOSTIC_JSON_LENGTH) {
      return encoded;
    }
  } catch {
    // Diagnostics must never replace the predicate failure with a serializer error.
  }
  return "{\"diagnostic\":\"unavailable\"}";
}

export interface ControlledMenuSnapshot {
  ariaExpanded: string | null;
  triggerState: string | null;
  menuPresent: boolean;
  menuRole: string | null;
  menuState: string | null;
}

export function controlledMenuReached(
  snapshot: ControlledMenuSnapshot,
  expanded: boolean,
): boolean {
  if (expanded) {
    return (
      snapshot.ariaExpanded === "true" &&
      snapshot.triggerState === "open" &&
      snapshot.menuPresent &&
      snapshot.menuRole === "menu" &&
      snapshot.menuState === "open"
    );
  }
  return (
    snapshot.ariaExpanded === "false" &&
    snapshot.triggerState === "closed" &&
    !snapshot.menuPresent
  );
}

export function sortedItemIds(ids: readonly string[]): string[] {
  return [...ids].sort();
}

export function sameSortedItemIds(
  expected: readonly string[],
  actual: readonly string[],
): boolean {
  const sortedActual = sortedItemIds(actual);
  return (
    expected.length === sortedActual.length &&
    expected.every((id, index) => id === sortedActual[index])
  );
}

export async function withCleanupPreservingPrimary<T>(
  operation: () => Promise<T>,
  cleanup: () => Promise<void>,
): Promise<T> {
  let result!: T;
  let primaryFailed = false;
  let primaryError: unknown;
  try {
    result = await operation();
  } catch (error) {
    primaryFailed = true;
    primaryError = error;
  }

  try {
    await cleanup();
  } catch (cleanupError) {
    if (primaryFailed) {
      throw new AggregateError(
        [primaryError, cleanupError],
        "History interaction and cleanup both failed",
      );
    }
    throw cleanupError;
  }

  if (primaryFailed) throw primaryError;
  return result;
}
