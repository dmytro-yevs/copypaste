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
import { InlineNotice } from "@/components/shared";
import {
    originName,
    wontSync,
    type OriginDevice,
} from "@/features/history/model/origin";
import { HighlightedCode } from "@/features/history/components/HighlightedCode";
import { ClipImageLoader } from "@/features/history/patterns/ClipImageLoader";
import { LibraryInspectorPanel } from "@/features/history/patterns/LibraryInspectorPanel";
import { useViewportMetrics } from "@/hooks/useViewportMetrics";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import { MONO_KINDS, absoluteTime, kindOf } from "@/lib/format";
import type { Item } from "@/lib/ipc";
import { EXPANDED_MIN_PX } from "@/lib/layoutBreakpoints";
import { kindLabel } from "@/lib/view";
import styles from "./ClipDetailDialog.module.css";

interface ClipDetailDialogProps {
    /** `null` closes the view. Resolved from the id every render, so an item
     *  deleted underneath the reader closes it rather than showing a ghost. */
    item: Item | null;
    origin: OriginDevice | null;
    initialExpanded?: boolean;
    fullContent: string | null;
    /** The whole-body read failed, so what is rendered is the row's truncated
     *  preview and the view must say so rather than pass a fragment off as the
     *  clipping. */
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
    const masked = item?.is_sensitive === true && !revealed;
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
    // Whole-item sensitive plaintext comes only from the reveal. This component
    // keeps no copy, so INV-11's expiry re-masks the view by itself.
    const body =
        potentialFinding !== null && !potentialRevealed
            ? potentialFinding.redacted_preview
            : revealed
              ? revealedContent
              : (fullContent ?? item?.content ?? null);
    const bodyIncomplete =
        fullContentFailed === true && item?.truncated === true && !revealed;
    const kind = item ? kindOf(item) : "text";
    const isImage = kind === "image";

    const meta = item
        ? [absoluteTime(item.created_at), kindLabel(kindOf(item))]
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

                        {masked ? (
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
                        ) : bodyIncomplete ? (
                            <div role="status" className={styles.unavailable}>
                                {t("history.detail.fullBodyUnavailable")}
                            </div>
                        ) : isImage && item ? (
                            <div
                                role="region"
                                aria-label={t("history.detail.image")}
                                tabIndex={0}
                                className={styles.imageRegion}
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
                            </div>
                        ) : kind === "code" || kind === "json" ? (
                            <div
                                role="region"
                                aria-label={t("history.detail.contents")}
                                tabIndex={0}
                                className={cn(
                                    styles.contentRegion,
                                    styles.codeRegion,
                                )}
                            >
                                <HighlightedCode
                                    content={body ?? ""}
                                    kind={kind}
                                    mode="expanded"
                                    ariaLabel={t("history.detail.contents")}
                                />
                            </div>
                        ) : (
                            <div
                                role="region"
                                aria-label={t("history.detail.contents")}
                                tabIndex={0}
                                className={styles.contentRegion}
                            >
                                <p
                                    className={cn(
                                        styles.body,
                                        MONO_KINDS.has(kind) && styles.mono,
                                    )}
                                >
                                    {body === null || body === ""
                                        ? t("history.detail.empty")
                                        : body}
                                </p>
                            </div>
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
                                <Icon name={isImage ? "image" : "copy"} />
                                {isImage
                                    ? t("history.detail.copyImage")
                                    : t("history.detail.copy")}
                            </Button>
                        </DialogFooter>
                    </>
                )}
            </DialogContent>
        </Dialog>
    );
}
