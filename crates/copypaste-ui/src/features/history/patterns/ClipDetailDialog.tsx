import { useEffect, useState } from "react";

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
    HighlightedCode,
    InlineNotice,
    PreviewSurface,
} from "@/components/shared";
import { ClipImageLoader } from "@/features/clip-content";
import {
    originName,
    wontSync,
    type OriginDevice,
} from "@/features/history/model/origin";
import { clipCopyAction } from "@/features/history/model/clipPresentation";
import { LibraryInspectorPanel } from "@/features/history/patterns/LibraryInspectorPanel";
import { useViewportMetrics } from "@/hooks/useViewportMetrics";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import {
    clipTypeMetadata,
    resolveClipBodyPresentation,
} from "@/lib/clipPresentation";
import { MONO_KINDS, absoluteTime, kindOf } from "@/lib/format";
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
    onCopy: (item: Item) => void;
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

    const revealed = item !== null && revealedContent !== null;
    const potentialFinding =
        item !== null && !item.is_sensitive ? item.sensitive_finding : null;
    const [shownFinding, setShownFinding] =
        useState<Item["sensitive_finding"]>(null);
    useEffect(() => {
        setExpanded(initialExpanded);
        setShownFinding(null);
    }, [initialExpanded, item?.id]);
    const potentialRevealed =
        potentialFinding !== null && shownFinding === potentialFinding;
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
        setExpanded(initialExpanded);
        setShownFinding(null);
        onClose();
    };

    return (
        <Dialog open={item !== null} onOpenChange={(open) => !open && close()}>
            <DialogContent
                presentation={sheet ? "sheet" : "modal"}
                showCloseButton={expanded}
                className={cn(
                    styles.dialog,
                    expanded ? styles.expanded : styles.normal,
                )}
                onCloseAutoFocus={(event) => {
                    event.preventDefault();
                    onReturnFocus();
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
                            onReveal={onReveal}
                            onHide={onHide}
                            onCopy={onCopy}
                            onTogglePin={onTogglePin}
                            onDelete={(target) => {
                                onDelete(target);
                                close();
                            }}
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
                        ) : kind === "image" && item ? (
                            <PreviewSurface
                                role="region"
                                aria-label={t("history.detail.image")}
                                tabIndex={0}
                                className={styles.imageRegion}
                                elevation="flat"
                                border="strong"
                                radius="md"
                                padding="compact"
                            >
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
                            </PreviewSurface>
                        ) : kind === "code" || kind === "json" ? (
                            <PreviewSurface
                                role="region"
                                aria-label={t("history.detail.contents")}
                                tabIndex={0}
                                className={cn(
                                    styles.contentRegion,
                                    styles.codeRegion,
                                )}
                                elevation="flat"
                                border="none"
                                radius="md"
                                padding="none"
                            >
                                <HighlightedCode
                                    content={content}
                                    kind={kind}
                                    mode="expanded"
                                    ariaLabel={t("history.detail.contents")}
                                />
                            </PreviewSurface>
                        ) : (
                            <PreviewSurface
                                role="region"
                                aria-label={t("history.detail.contents")}
                                tabIndex={0}
                                className={styles.contentRegion}
                                elevation="flat"
                                border="strong"
                                radius="md"
                                padding="compact"
                            >
                                <p
                                    className={cn(
                                        styles.body,
                                        MONO_KINDS.has(kind) && styles.mono,
                                    )}
                                >
                                    {content === ""
                                        ? t("history.detail.empty")
                                        : content}
                                </p>
                            </PreviewSurface>
                        )}

                        <DialogFooter>
                            {potentialFinding !== null && (
                                <Button
                                    variant="secondary"
                                    aria-pressed={potentialRevealed}
                                    onClick={() =>
                                        setShownFinding(
                                            potentialRevealed
                                                ? null
                                                : potentialFinding,
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
                                <Button variant="secondary" onClick={onHide}>
                                    <Icon name="eyeOff" />
                                    {t("history.detail.hide")}
                                </Button>
                            )}
                            <Button
                                onClick={() => {
                                    if (item) onCopy(item);
                                    close();
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
