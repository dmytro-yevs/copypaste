/**
 * Check DOM containment (crossing shadow boundaries via `.host`, like `elementContains.ts`)
 * for many candidate elements in a single round trip instead of one `execute()` call per
 * candidate. Kept as a separate self-contained function (rather than sharing a helper with
 * `elementContains.ts`) because `browser.execute()` serializes only the passed function's
 * own source text — it cannot reference helpers defined outside of it.
 */
export default function elementsContainedIn(scope: HTMLElement, elements: HTMLElement[]): boolean[];
//# sourceMappingURL=elementsContainedIn.d.ts.map