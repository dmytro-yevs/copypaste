import { useEffect, useRef, useState } from "react";

import {
    Button,
    Dialog,
} from "@/components/ui";
import { StateView } from "@/components/shared/StateView";
import { ClipBodyNotices, ClipBodyView, ClipPotentialRevealButton } from "@/features/history/patterns/ClipBodyPresentation";
import { originName, wontSync, type OriginDevice } from "@/lib/itemOrigin";
import { clipboardCopyPresentation, clipCopyAction } from "@/features/history/model/clipPresentation";
import { useClipboardWriteAvailability } from "@/hooks/useClipboardWriteAvailability";
import { LibraryInspectorPanel } from "@/features/history/patterns/LibraryInspectorPanel";
import { useViewportMetrics } from "@/hooks/useViewportMetrics";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import {
    clipTypeMetadata,
    resolveClipBodyPresentation,
} from "@/lib/clipPresentation";
import { absoluteTime, kindOf } from "@/lib/format";
import type { Item } from "@/lib/ipc";
import { EXPANDED_MIN_PX } from "@/lib/layoutBreakpoints";
import styles from "./ClipDetailDialog.module.css";

interface ClipDetailDialogProps {
    /** `null` closes the view. Resolved from the id every render, so an item
     *  deleted underneath the reader closes it rather than showing a ghost. */
    item: Item | null;
    origin: OriginDevice | null;
    initialExpanded?: boolean;
    fullContent: string | null;
    /** A failed whole-body read renders unavailable, never a preview fragment
     *  presented as complete content. */
    fullContentFailed?: boolean;
    revealedContent: string | null;
    revealPending: boolean;
    onReveal: (item: Item) => void;
    onHide: () => void;
    onCopy: (item: Item) => Promise<unknown>;
    onTogglePin: (item: Item) => void;
    onDelete: (item: Item) => void;
    onClose: () => void;
    /** Where focus goes when the view closes. The trigger is a row inside a
     *  virtualised list and may not exist by then, and Radix's own restore would
     *  drop focus on `<body>`. */
    onReturnFocus: () => void;
}

export function ClipDetailDialog({
    item,
    origin,
    initialExpanded = false,
    fullContent,
    fullContentFailed,
    revealedContent,
    revealPending,
    onReveal,
    onHide,
    onCopy,
    onTogglePin,
    onDelete,
    onClose,
    onReturnFocus,
}: ClipDetailDialogProps) {
    const { t } = useTranslation();
    const sheet = useViewportMetrics().width < EXPANDED_MIN_PX;
    const [expanded, setExpanded] = useState(initialExpanded);
    const [copying, setCopying] = useState(false);
    const copyingRef = useRef(false);
    const copyGenerationRef = useRef(0);
    const contentRef = useRef<HTMLDivElement>(null);
    const availability = useClipboardWriteAvailability(item?.content_type ?? null);
    const copyAvailability = clipboardCopyPresentation(
        availability.isPending
            ? { status: "loading" }
            : availability.isError
              ? { status: "failed" }
              : { status: "resolved", availability: availability.data },
    );

    const revealed = item !== null && revealedContent !== null;
    const potentialFinding =
        item !== null && !item.is_sensitive ? item.sensitive_finding : null;
    const [shownFinding, setShownFinding] = useState<{
        id: string;
        finding: NonNullable<Item["sensitive_finding"]>;
    } | null>(null);
    useEffect(() => {
        setExpanded(initialExpanded);
        setShownFinding(null);
    }, [initialExpanded, item?.id]);
    useEffect(() => {
        copyGenerationRef.current += 1;
        copyingRef.current = false;
        setCopying(false);
    }, [item?.id]);
    const potentialRevealed =
        potentialFinding !== null &&
        shownFinding !== null &&
        shownFinding.id === item?.id &&
        shownFinding.finding === potentialFinding;
    const kind = item ? kindOf(item) : "text";
    // Revealed plaintext remains an ephemeral argument from useReveal; this
    // pure resolver retains no copy outside the current render.
    const body = item
        ? resolveClipBodyPresentation({
              item,
              fullContent,
              fullContentFailed: fullContentFailed === true,
              revealedContent,
              showPotentialSensitiveOriginal: potentialRevealed,
          })
        : null;
    const copyAction = clipCopyAction(kind);

    const meta = item
        ? [absoluteTime(item.created_at), clipTypeMetadata(kind).label]
        : [];
    if (item && origin !== null) {
        meta.push(`${t("history.row.fromPrefix")} ${originName(origin)}`);
    }

    const close = () => {
        if (copyingRef.current) return;
        setExpanded(initialExpanded);
        setShownFinding(null);
        onClose();
    };

    const startCopy = (target: Item, closeAfterSuccess: boolean): void => {
        if (copyingRef.current || !copyAvailability.canCopy) return;
        const generation = copyGenerationRef.current;
        copyingRef.current = true;
        setCopying(true);
        void Promise.resolve()
            .then(() => onCopy(target))
            .then(() => {
                if (generation !== copyGenerationRef.current) return;
                copyingRef.current = false;
                setCopying(false);
                if (closeAfterSuccess) close();
            })
            .catch(() => {
                if (generation !== copyGenerationRef.current) return;
                copyingRef.current = false;
                setCopying(false);
            });
    };

    return (
        <Dialog
            open={item !== null}
            onOpenChange={(open) => !open && close()}
            title={t("history.detail.title")}
            description={expanded ? meta.join(" · ") : undefined}
            headerHidden={!expanded}
            showCloseButton={expanded}
            contentProps={{
                ref: contentRef,
                presentation: sheet ? "sheet" : "modal",
                "aria-busy": copying || undefined,
                className: cn(styles.dialog, expanded ? styles.expanded : styles.normal),
                onCloseAutoFocus: (event) => {
                    event.preventDefault();
                    onReturnFocus();
                },
                onEscapeKeyDown: (event) => {
                    if (copyingRef.current) event.preventDefault();
                },
                onPointerDownOutside: (event) => {
                    if (!copyingRef.current) return;
                    event.preventDefault();
                    requestAnimationFrame(() => {
                        if (copyingRef.current) contentRef.current?.focus();
                    });
                },
                onInteractOutside: (event) => {
                    if (copyingRef.current) event.preventDefault();
                },
            }}
            footer={expanded ? <>
                {potentialFinding !== null && <ClipPotentialRevealButton
                    revealed={potentialRevealed}
                    disabled={copying}
                    onClick={() => setShownFinding(potentialRevealed ? null : {
                        id: item!.id,
                        finding: potentialFinding,
                    })}
                />}
                {revealed && <Button variant="secondary" icon="eyeOff" disabled={copying} onClick={onHide}>
                    {t("history.detail.hide")}
                </Button>}
                {item && <Button
                    variant="secondary"
                    icon={item.pinned ? "unpin" : "pin"}
                    disabled={copying}
                    aria-pressed={item.pinned}
                    onClick={() => onTogglePin(item)}
                >{t(item.pinned ? "history.row.unpin" : "history.row.pin")}</Button>}
                {item && <Button
                    variant="secondary"
                    icon="trash"
                    disabled={copying}
                    onClick={() => { onDelete(item); close(); }}
                >{t("history.row.delete")}</Button>}
                <Button
                    icon={copyAction.icon}
                    disabled={copying || !copyAvailability.canCopy}
                    onClick={() => { if (item) startCopy(item, true); }}
                >{copyAction.label}</Button>
            </> : undefined}
        >
            {!expanded && item ? <LibraryInspectorPanel
                item={item}
                origin={origin}
                revealedContent={revealedContent}
                fullContent={fullContent}
                fullContentFailed={fullContentFailed === true}
                revealPending={revealPending}
                copyPending={copying}
                onReveal={onReveal}
                onHide={onHide}
                onCopy={(target) => startCopy(target, false)}
                onTogglePin={onTogglePin}
                onDelete={(target) => { onDelete(target); close(); }}
                onOpenReader={() => setExpanded(true)}
                onClose={close}
            /> : <>
                {item && wontSync(item) && <StateView
                    mode="warning"
                    placement="inline"
                    role="none"
                    icon="cloudOff"
                    title={t("history.row.wontSync")}
                />}
                <ClipBodyNotices
                    reason={copyAvailability.reason}
                    canRetry={copyAvailability.canRetry}
                    onRetry={() => void availability.refetch()}
                    potentialFinding={potentialFinding !== null}
                />
                {item && body && <ClipBodyView
                    mode="reader"
                    item={item}
                    kind={kind}
                    body={body}
                    copyPending={copying}
                    revealPending={revealPending}
                    onReveal={onReveal}
                />}
            </>}
        </Dialog>
    );
}
