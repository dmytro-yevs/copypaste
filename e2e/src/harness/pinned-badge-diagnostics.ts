import type { Browser } from "./webview-guard.js";
import { ROW } from "./ui.js";

export interface PinnedBadgeRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface PinnedBadgeSnapshot {
  expectedIds: readonly string[];
  renderedRowIds: readonly string[];
  rows: ReadonlyArray<{
    id: string;
    rowCount: number;
    badgeCount: number;
    row: {
      connected: boolean;
      displayed: boolean;
      rect: PinnedBadgeRect;
    } | null;
    badge: {
      connected: boolean;
      displayed: boolean;
      rect: PinnedBadgeRect;
      display: string;
      visibility: string;
      opacity: string;
    } | null;
  }>;
}

export function readPagePinnedBadgeSnapshot(
  ids: readonly string[],
  rowSelector: string,
): PinnedBadgeSnapshot {
  const rectOf = (element: HTMLElement): PinnedBadgeRect => {
    const rect = element.getBoundingClientRect();
    return {
      left: rect.left,
      top: rect.top,
      width: rect.width,
      height: rect.height,
    };
  };
  const stateOf = (element: HTMLElement) => {
    const rect = rectOf(element);
    let displayed = element.isConnected && rect.width > 0 && rect.height > 0;
    for (
      let ancestor: HTMLElement | null = element;
      displayed && ancestor !== null;
      ancestor = ancestor.parentElement
    ) {
      const style = getComputedStyle(ancestor);
      displayed =
        style.display !== "none" &&
        style.visibility !== "hidden" &&
        style.visibility !== "collapse" &&
        style.opacity !== "0";
    }
    const style = getComputedStyle(element);
    return {
      connected: element.isConnected,
      displayed,
      rect,
      display: style.display,
      visibility: style.visibility,
      opacity: style.opacity,
    };
  };
  const renderedRows = Array.from(
    document.querySelectorAll<HTMLElement>(rowSelector),
  );

  return {
    expectedIds: [...ids],
    renderedRowIds: renderedRows.map((row) =>
      row.id.replace(/^history-row-/, ""),
    ),
    rows: ids.map((id) => {
      const rowMatches = renderedRows.filter(
        (row) => row.id === `history-row-${id}`,
      );
      const badges = rowMatches.flatMap((row) =>
        Array.from(row.querySelectorAll<HTMLElement>('[title="Pinned"]')),
      );
      return {
        id,
        rowCount: rowMatches.length,
        badgeCount: badges.length,
        row: rowMatches.length === 1 ? stateOf(rowMatches[0]!) : null,
        badge: badges.length === 1 ? stateOf(badges[0]!) : null,
      };
    }),
  };
}

/**
 * Read pinned badges without retaining WebDriver elements across a render.
 * Rows can reorder as the pin refresh arrives, so all identity and geometry
 * facts must come from one page-side DOM evaluation.
 */
export async function pinnedBadgeSnapshot(
  browser: Browser,
  expectedIds: readonly string[],
): Promise<PinnedBadgeSnapshot> {
  return (await browser.execute(
    readPagePinnedBadgeSnapshot,
    [...expectedIds],
    ROW,
  )) as PinnedBadgeSnapshot;
}
