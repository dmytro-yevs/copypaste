import {
    memo,
    useCallback,
    useEffect,
    useMemo,
    useRef,
    type PointerEvent as ReactPointerEvent,
    type ReactNode,
} from "react";

import { Button, SelectionControl, Surface } from "@/components/ui";
import { ClipCardBody } from "@/features/history/components/ClipCardBody";
import { SourceMeta } from "@/features/history/components/SourceMeta";
import { SourceMetaBadge } from "@/features/history/components/SourceMetaBadge";
import {
    clipSourceMetadata,
    rowLabel,
} from "@/features/history/model/clipPresentation";
import {
    originName,
    wontSync,
    type OriginDevice,
} from "@/features/history/model/origin";
import { HISTORY_LAYOUT_METRICS } from "@/features/history/model/virtualizationMetrics";
import { useTranslation } from "@/i18n";
import { kindOf, previewOf } from "@/lib/format";
import type { Item } from "@/lib/ipc";
import styles from "./ClipCard.module.css";

const LONG_PRESS_MS = 450;

export type CardSelectionIntent = "toggle" | "range";

interface ClipCardProps {
    item: Item;
    state: "default" | "active" | "checked" | "dragging" | "pending";
    revealedContent: string | null;
    revealPending: boolean;
    origin: OriginDevice | null;
    selectionActive: boolean;
    checked: boolean;
    previewLines: number;
    imagePreview?: ReactNode;
    onSelection: (item: Item, intent: CardSelectionIntent) => void;
    onActivate: (item: Item) => void;
}

interface PressState {
    pointerId: number;
    x: number;
    y: number;
    timer: number;
}

function ClipCardImpl({
    item,
    state,
    revealedContent,
    revealPending,
    origin,
    selectionActive,
    checked,
    previewLines,
    imagePreview,
    onSelection,
    onActivate,
}: ClipCardProps) {
    const { t: tr } = useTranslation();
    const press = useRef<PressState | null>(null);
    const selectedByLongPress = useRef(false);
    const resetLongPress = useRef<number | null>(null);
    const { kind, preview } = useMemo(
        () => ({
            kind: kindOf(item),
            preview: item.content === null ? "" : previewOf(item.content),
        }),
        [item],
    );
    const source = clipSourceMetadata(item);
    const revealed = revealedContent !== null;
    const masked = item.is_sensitive && !revealed;
    const potentialFinding = item.is_sensitive ? null : item.sensitive_finding;
    const stranded = wontSync(item);
    const label = rowLabel(
        item,
        origin ? originName(origin) : null,
        preview,
        tr,
    );
    const body =
        potentialFinding !== null
            ? potentialFinding.redacted_preview
            : revealedContent !== null
              ? revealedContent
              : preview;

    const stopPress = useCallback(() => {
        if (press.current !== null) {
            window.clearTimeout(press.current.timer);
            press.current = null;
        }
        window.removeEventListener("scroll", stopPress, true);
    }, []);

    useEffect(
        () => () => {
            stopPress();
            if (resetLongPress.current !== null)
                window.clearTimeout(resetLongPress.current);
        },
        [stopPress],
    );

    const beginPress = (event: ReactPointerEvent<HTMLButtonElement>) => {
        if (event.pointerType !== "touch") return;
        if (!event.isPrimary || press.current !== null) {
            stopPress();
            return;
        }
        const pointerId = event.pointerId;
        const timer = window.setTimeout(() => {
            if (press.current?.pointerId !== pointerId) return;
            selectedByLongPress.current = true;
            onSelection(item, "toggle");
            navigator.vibrate?.(10);
            stopPress();
            if (resetLongPress.current !== null)
                window.clearTimeout(resetLongPress.current);
            resetLongPress.current = window.setTimeout(() => {
                selectedByLongPress.current = false;
            }, 900);
        }, LONG_PRESS_MS);
        press.current = {
            pointerId,
            x: event.clientX,
            y: event.clientY,
            timer,
        };
        window.addEventListener("scroll", stopPress, true);
    };

    const movePress = (event: ReactPointerEvent<HTMLButtonElement>) => {
        const active = press.current;
        if (active === null || active.pointerId !== event.pointerId) return;
        if (
            Math.abs(event.clientX - active.x) >
                HISTORY_LAYOUT_METRICS.interaction.longPressSlopPx ||
            Math.abs(event.clientY - active.y) >
                HISTORY_LAYOUT_METRICS.interaction.longPressSlopPx
        ) {
            stopPress();
        }
    };

    const choose = (intent: CardSelectionIntent) => onSelection(item, intent);

    return (
        <Surface
            className={styles.root}
            data-state={state}
            data-kind={kind}
            data-selection-active={selectionActive || undefined}
            data-checked={checked || undefined}
            aria-busy={masked && revealPending ? true : undefined}
        >
            <Button
                variant="ghost"
                size="md"
                className={styles.hit}
                aria-label={label}
                tabIndex={state === "active" ? 0 : -1}
                onPointerDown={beginPress}
                onPointerMove={movePress}
                onPointerUp={stopPress}
                onPointerCancel={stopPress}
                onContextMenu={(event) => {
                    if (selectedByLongPress.current) event.preventDefault();
                }}
                onKeyDown={(event) => {
                    if (event.key !== " ") return;
                    event.preventDefault();
                    event.stopPropagation();
                    choose("toggle");
                }}
                onClick={(event) => {
                    if (selectedByLongPress.current) {
                        event.preventDefault();
                        return;
                    }
                    if (event.shiftKey) choose("range");
                    else if (selectionActive || event.metaKey || event.ctrlKey)
                        choose("toggle");
                    else onActivate(item);
                }}
            />
            <span className={styles.selection}>
                <SelectionControl
                    hitSize={selectionActive ? "comfortable" : "compact"}
                    checked={checked}
                    tabIndex={state === "active" && selectionActive ? 0 : -1}
                    aria-label={`${tr("history.row.selectPrefix")} ${label}`}
                    onKeyDown={(event) => {
                        if (event.key === " " || event.key === "Enter")
                            event.stopPropagation();
                    }}
                    onClick={(event) => {
                        event.stopPropagation();
                        choose(event.shiftKey ? "range" : "toggle");
                    }}
                />
            </span>
            <div className={styles.content}>
                <div className={styles.body}>
                    <ClipCardBody
                        kind={kind}
                        masked={masked}
                        content={body}
                        previewLines={previewLines}
                        imagePreview={imagePreview}
                    />
                </div>
                <SourceMeta
                    item={item}
                    source={source}
                    origin={origin}
                    kind={kind}
                    content={body}
                    extras={
                        <>
                            {item.pinned && (
                                <SourceMetaBadge
                                    icon="pin"
                                    label={tr("history.row.pinnedBadge")}
                                />
                            )}
                            {potentialFinding !== null && (
                                <SourceMetaBadge
                                    icon="sensitive"
                                    tone="warning"
                                    label={tr(
                                        "history.row.potentialSensitiveBadge",
                                    )}
                                    title={tr(
                                        "history.row.potentialSensitiveWarning",
                                    )}
                                />
                            )}
                            {stranded && (
                                <SourceMetaBadge
                                    icon="cloudOff"
                                    tone="warning"
                                    label={tr("history.row.wontSyncBadge")}
                                    title={tr("history.row.wontSync")}
                                />
                            )}
                        </>
                    }
                />
            </div>
        </Surface>
    );
}

export const ClipCard = memo(ClipCardImpl);
