import {
    historyKindFilterLabel,
} from "@/features/history/model/clipPresentation";
import { clipTypeMetadata } from "@/lib/clipPresentation";
import type { ClipPresentationIcon } from "@/lib/clipSourcePresentation";
import { type OriginDevice, originName } from "@/lib/itemOrigin";
import { t } from "@/i18n";
import { FILTERABLE_KINDS, sortLabel } from "@/lib/view";

type LibraryToolbarIcon =
    | ClipPresentationIcon
    | "devices"
    | "mobile"
    | "monitor"
    | "sortAscending"
    | "sortDescending"
    | "tablet";

export interface LibraryToolbarOption {
    readonly value: string;
    readonly label: string;
    readonly icon: LibraryToolbarIcon;
}

function deviceIcon(kind: OriginDevice["kind"]): LibraryToolbarIcon {
    if (kind === "phone") return "mobile";
    if (kind === "tablet") return "tablet";
    if (kind === "desktop") return "monitor";
    return "devices";
}

export function historyKindOptions(): readonly LibraryToolbarOption[] {
    return FILTERABLE_KINDS.map((kind) => ({
        value: kind,
        label: historyKindFilterLabel(kind),
        icon: clipTypeMetadata(kind).icon,
    }));
}

export function historyDeviceOptions(
    origins: readonly OriginDevice[],
): readonly LibraryToolbarOption[] {
    return origins.map((origin) => ({
        value: origin.id,
        label: originName(origin),
        icon: deviceIcon(origin.kind),
    }));
}

export function historySortOptions(): readonly LibraryToolbarOption[] {
    return [
        {
            value: "newest",
            label: sortLabel("newest"),
            icon: "sortDescending",
        },
        {
            value: "oldest",
            label: sortLabel("oldest"),
            icon: "sortAscending",
        },
    ];
}

export function historyCountNumber(
    filtered: boolean,
    visible: number,
    total: number | undefined,
): number {
    return filtered || total === undefined ? visible : total;
}

export function historyCount(
    filtered: boolean,
    visible: number,
    total: number | undefined,
): string {
    return t("history.search.count", {
        count: historyCountNumber(filtered, visible, total),
    });
}
