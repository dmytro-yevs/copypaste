import {
    DEFAULT_PREVIEW_LINES,
    MAX_PREVIEW_LINES,
    MIN_PREVIEW_LINES,
    previewLineCount,
} from "@/lib/previewDensity";
import type { Kind } from "@/lib/format";
import {
    EXTRA_SMALL_MAX_PX,
    HISTORY_INSPECTOR_MIN_PX,
} from "@/lib/layoutBreakpoints";

export { DEFAULT_PREVIEW_LINES, MAX_PREVIEW_LINES, MIN_PREVIEW_LINES };

export const HISTORY_LAYOUT_METRICS = {
    inspector: {
        visibleAtPx: HISTORY_INSPECTOR_MIN_PX,
        primaryMinPx: 390,
        minPx: 278,
        defaultPx: 322,
        widePx: 390,
        maxSize: "50%",
    },
    toolbar: {
        compactSearchMaxPx: EXTRA_SMALL_MAX_PX,
    },
    row: {
        paddingVerticalPx: 20,
        bodyLinePx: 21,
        metaLinePx: 18,
        metaMarginPx: 8,
        outerChromePx: 10,
        imageHeightMultiplier: 3.75,
    },
    virtualizer: {
        overscanPx: 240,
        loadMoreThresholdPx: 300,
        estimatePx: {
            group: 34,
            code: 156,
            imageExpanded: 328,
            imageCompact: 298,
            file: 124,
            color: 128,
            secret: 116,
        },
    },
    interaction: {
        longPressSlopPx: 8,
    },
} as const;

const rowMetrics = HISTORY_LAYOUT_METRICS.row;
const singleLineFloor =
    rowMetrics.bodyLinePx +
    rowMetrics.metaMarginPx +
    rowMetrics.metaLinePx +
    rowMetrics.paddingVerticalPx;

export const ROW_PAD_V = rowMetrics.paddingVerticalPx;
export const TITLE_LINE_PX = rowMetrics.bodyLinePx;
export const META_LINE_PX = rowMetrics.metaLinePx;
export const META_MARGIN_PX = rowMetrics.metaMarginPx;
export const SINGLE_LINE_FLOOR = singleLineFloor;
export const IMAGE_PREVIEW_HEIGHT_MULTIPLIER = rowMetrics.imageHeightMultiplier;
export const OVERSCAN_PX = HISTORY_LAYOUT_METRICS.virtualizer.overscanPx;

export function rowHeight(previewLines: number): number {
    return (
        singleLineFloor +
        (previewLineCount(previewLines) - 1) * rowMetrics.bodyLinePx
    );
}

export function rowPreviewHeight(previewLines: number): number {
    return previewLineCount(previewLines) * rowMetrics.bodyLinePx;
}

export function imagePreviewHeight(previewLines: number): number {
    return rowPreviewHeight(previewLines) * rowMetrics.imageHeightMultiplier;
}

export function imageRowHeight(previewLines: number): number {
    return (
        imagePreviewHeight(previewLines) +
        rowMetrics.metaMarginPx +
        rowMetrics.metaLinePx +
        rowMetrics.paddingVerticalPx
    );
}

export function overscanRows(previewLines: number): number {
    return Math.ceil(
        HISTORY_LAYOUT_METRICS.virtualizer.overscanPx / rowHeight(previewLines),
    );
}

/** INV-5: each initial size reserves the full rendered variant cap. Compact
 * images use the largest width below the shared shell boundary. Mounted rows
 * replace reservations with measurements; none of these values sizes a card. */
export function historyRowEstimate(
    kind: Kind | "group",
    previewLines: number,
    compact = false,
    _metadataUnits = 1,
): number {
    const estimates = HISTORY_LAYOUT_METRICS.virtualizer.estimatePx;
    if (kind === "group") return estimates.group;
    if (kind === "image") {
        return compact ? estimates.imageCompact : estimates.imageExpanded;
    }
    if (kind === "code" || kind === "json") {
        return estimates.code;
    }
    if (kind === "file" || kind === "path" || kind === "url") {
        return estimates.file;
    }
    if (kind === "color") return estimates.color;
    if (kind === "secret") return estimates.secret;
    return rowHeight(previewLines) + rowMetrics.outerChromePx;
}
