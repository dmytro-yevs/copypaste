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
