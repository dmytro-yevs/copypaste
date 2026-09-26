import { useEffect, useRef, useState } from "react";

import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
    Icon,
    VisuallyHidden,
} from "@/components/ui";
import {
    InlineNotice,
    PreviewSurface,
} from "@/components/shared";
import { ClipImageLoader } from "@/features/clip-content";
import { InspectorPreview } from "@/features/history/components/InspectorPreview";
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
    const content = body?.state === "content" ? body.content : "";
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
        <Dialog open={item !== null} onOpenChange={(open) => !open && close()}>
            <DialogContent
                ref={contentRef}
                presentation={sheet ? "sheet" : "modal"}
                showCloseButton={expanded}
                aria-busy={copying || undefined}
                className={cn(
                    styles.dialog,
                    expanded ? styles.expanded : styles.normal,
                )}
                onCloseAutoFocus={(event) => {
                    event.preventDefault();
                    onReturnFocus();
                }}
                onEscapeKeyDown={(event) => {
                    if (copyingRef.current) event.preventDefault();
                }}
                onPointerDownOutside={(event) => {
                    if (!copyingRef.current) return;
                    event.preventDefault();
                    requestAnimationFrame(() => {
                        if (copyingRef.current) contentRef.current?.focus();
                    });
                }}
                onInteractOutside={(event) => {
                    if (copyingRef.current) event.preventDefault();
                }}
            >
                {!expanded && item ? (
                    <>
                        <VisuallyHidden asChild>
                            <DialogTitle>
                                {t("history.detail.title")}
                            </DialogTitle>
                        </VisuallyHidden>
                        <LibraryInspectorPanel
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
                            onDelete={(target) => {
                                onDelete(target);
                                close();
                            }}
                            onOpenReader={() => setExpanded(true)}
                            onClose={close}
                        />
                    </>
                ) : (
                    <>
                        <DialogHeader>
                            <DialogTitle>
                                {t("history.detail.title")}
                            </DialogTitle>
                            <DialogDescription>
                                {meta.join(" · ")}
                            </DialogDescription>
                        </DialogHeader>

                        {item && wontSync(item) && (
                            <InlineNotice tone="warning" icon="cloudOff">
                                {t("history.row.wontSync")}
                            </InlineNotice>
                        )}

                        {copyAvailability.reason !== null && (
                            <InlineNotice
                                role="status"
                                tone={copyAvailability.canRetry ? "warning" : "neutral"}
                                icon="info"
                                action={copyAvailability.canRetry ? (
                                    <Button variant="secondary" size="sm" onClick={() => void availability.refetch()}>
                                        {t("history.copyAvailability.retry")}
                                    </Button>
                                ) : undefined}
                            >
                                {copyAvailability.reason}
                            </InlineNotice>
                        )}

                        {potentialFinding !== null && (
                            <InlineNotice
                                role="status"
                                tone="warning"
                                icon="sensitive"
                            >
                                {t("history.row.potentialSensitiveWarning")}
                            </InlineNotice>
                        )}

                        {body?.state === "masked" ? (
                            <Button
                                type="button"
                                variant="ghost"
                                disabled={copying}
                                aria-label={t("history.row.sensitiveReveal")}
                                aria-busy={revealPending || undefined}
                                className={styles.masked}
                                onClick={() =>
                                    item && !revealPending && onReveal(item)
                                }
                            >
                                <span
                                    aria-hidden="true"
                                    className={styles.redactions}
                                >
                                    <span className={styles.redactionLong} />
                                    <span className={styles.redactionShort} />
                                    <span className={styles.redactionMedium} />
                                </span>
                                {revealPending && (
                                    <Icon
                                        name="spinner"
                                        className={styles.spinner}
                                    />
                                )}
                            </Button>
                        ) : body?.state === "unavailable" ? (
                            <PreviewSurface
                                role="status"
                                className={styles.unavailable}
                                elevation="flat"
                                border="subtle"
                                radius="md"
                                padding="roomy"
                            >
                                {t("history.detail.fullBodyUnavailable")}
                            </PreviewSurface>
                        ) : body?.state === "content" && body.source === "preview" ? (
                            <PreviewSurface
                                role="status"
                                className={styles.unavailable}
                                elevation="flat"
                                border="subtle"
                                radius="md"
                                padding="roomy"
                            >
                                {t("history.empty.loading.title")}
                            </PreviewSurface>
                        ) : (
                            <PreviewSurface
                                className={styles.contentRegion}
                                elevation="flat"
                                border={
                                    kind === "code" || kind === "json"
                                        ? "none"
                                        : "strong"
                                }
                                radius="md"
                                padding="compact"
                            >
                                <InspectorPreview
                                    mode="reader"
                                    kind={kind}
                                    ariaLabel={t(
                                        kind === "image"
                                            ? "history.detail.image"
                                            : "history.detail.contents",
                                    )}
                                    content={
                                        content === ""
                                            ? t("history.detail.empty")
                                            : content
                                    }
                                    imagePreview={
                                        kind === "image" && item ? (
                                            <ClipImageLoader
                                                id={item.id}
                                                size="detail"
                                                loadingLabel={t(
                                                    "history.detail.imageLoading",
                                                )}
                                                failureLabel={t(
                                                    "history.detail.imageUnavailable",
                                                )}
                                                title={t("history.detail.image")}
                                            />
                                        ) : undefined
                                    }
                                />
                            </PreviewSurface>
                        )}

                        <DialogFooter>
                            {potentialFinding !== null && (
                                <Button
                                    variant="secondary"
                                    disabled={copying}
                                    aria-pressed={potentialRevealed}
                                    onClick={() =>
                                        setShownFinding(
                                            potentialRevealed
                                                ? null
                                                : { id: item!.id, finding: potentialFinding },
                                        )
                                    }
                                >
                                    {potentialRevealed ? (
                                        <Icon name="eyeOff" />
                                    ) : (
                                        <Icon name="eye" />
                                    )}
                                    {t(
                                        potentialRevealed
                                            ? "history.row.hideOriginal"
                                            : "history.row.showOriginal",
                                    )}
                                </Button>
                            )}
                            {revealed && (
                                <Button variant="secondary" disabled={copying} onClick={onHide}>
                                    <Icon name="eyeOff" />
                                    {t("history.detail.hide")}
                                </Button>
                            )}
                            {item ? (
                                <Button
                                    variant="secondary"
                                    disabled={copying}
                                    aria-pressed={item.pinned}
                                    onClick={() => onTogglePin(item)}
                                >
                                    <Icon name={item.pinned ? "unpin" : "pin"} />
                                    {t(
                                        item.pinned
                                            ? "history.row.unpin"
                                            : "history.row.pin",
                                    )}
                                </Button>
                            ) : null}
                            {item ? (
                                <Button
                                    variant="secondary"
                                    disabled={copying}
                                    onClick={() => {
                                        onDelete(item);
                                        close();
                                    }}
                                >
                                    <Icon name="trash" />
                                    {t("history.row.delete")}
                                </Button>
                            ) : null}
                            <Button
                                disabled={copying || !copyAvailability.canCopy}
                                onClick={() => {
                                    if (item) startCopy(item, true);
                                }}
                            >
                                <Icon name={copyAction.icon} />
                                {copyAction.label}
                            </Button>
                        </DialogFooter>
                    </>
                )}
            </DialogContent>
        </Dialog>
    );
}
