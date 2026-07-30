/**
 * Check, for each of `elementIds`, whether it's a real DOM descendant of `scope`
 * (crossing shadow boundaries via `.host`), in a single round trip with per-element
 * fallback.
 */
export declare function checkElementsContainedIn(browser: WebdriverIO.Browser, scope: string, elementIds: string[], contextId?: string, onElementCheckError?: (elementId: string) => void): Promise<boolean[]>;
/**
 * Check, for each of `elementIds`, whether it's still connected to the live DOM,
 * in a single round trip with per-element fallback. Stale references that can't be
 * resolved at all count as not connected.
 */
export declare function checkElementsConnected(browser: WebdriverIO.Browser, elementIds: string[], contextId?: string, onElementCheckError?: (elementId: string) => void): Promise<boolean[]>;
//# sourceMappingURL=elementChecks.d.ts.map