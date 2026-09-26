import { useEffect, useLayoutEffect, useRef, useState } from "react";

import {
    ActionButton,
    DeviceMeta,
    InspectorShell,
    MetadataLabel,
    MetadataList,
    MetadataRow,
    MetadataValue,
    InlineNotice,
    PreviewSurface,
    TruncatedValue,
} from "@/components/shared";
import { ClipImageLoader } from "@/features/clip-content";
import { Button, Icon, iconComponent } from "@/components/ui";
import { InspectorPreview } from "@/features/history/components/InspectorPreview";
import { clipboardCopyPresentation, clipCopyAction } from "@/features/history/model/clipPresentation";
import { useClipboardWriteAvailability } from "@/hooks/useClipboardWriteAvailability";
import { originName, type OriginDevice } from "@/lib/itemOrigin";
import { SourceAppIcon } from "@/features/source-apps";
import { useTranslation } from "@/i18n";
import { absoluteTime, kindOf } from "@/lib/format";
import { clipTypeMetadata, resolveClipBodyPresentation } from "@/lib/clipPresentation";
import { clipSourceMetadata } from "@/lib/clipSourcePresentation";
import type { Item } from "@/lib/ipc";
import styles from "./LibraryInspectorPanel.module.css";

interface LibraryInspectorPanelProps {
    item: Item | null;
    origin: OriginDevice | null;
    revealedContent: string | null;
    fullContent: string | null;
    fullContentFailed: boolean;
    revealPending: boolean;
    copyPending?: boolean;
    onReveal: (item: Item) => void;
    onHide: () => void;
    onCopy: (item: Item) => void;
    onTogglePin: (item: Item) => void;
    onDelete: (item: Item) => void;
    onOpenReader: (item: Item, trigger: HTMLElement) => void;
    onClose: () => void;
}

export function LibraryInspectorPanel({
    item,
    origin,
    revealedContent,
    fullContent,
    fullContentFailed,
    revealPending,
    copyPending = false,
    onReveal,
    onHide,
    onCopy,
    onTogglePin,
    onDelete,
    onOpenReader,
    onClose,
}: LibraryInspectorPanelProps) {
    const { t } = useTranslation();
    const copyButtonRef = useRef<HTMLButtonElement>(null);
    const copyFocusRef = useRef<{
        itemId: string;
        ownedFocus: boolean;
        sawPending: boolean;
        abandoned: boolean;
    } | null>(null);
    const [shownFinding, setShownFinding] = useState<{
        id: string;
        finding: NonNullable<Item["sensitive_finding"]>;
    } | null>(null);
    useEffect(() => setShownFinding(null), [item?.id]);
    useLayoutEffect(() => {
        const attempt = copyFocusRef.current;
        if (!attempt) return;
        if (item?.id !== attempt.itemId) {
            copyFocusRef.current = null;
            return;
        }
        if (copyPending) {
            attempt.sawPending = true;
            const abandonPointer = (event: PointerEvent) => {
                if (
                    !(event.target instanceof Node) ||
                    !copyButtonRef.current?.parentElement?.contains(event.target)
                ) {
                    attempt.abandoned = true;
                }
            };
            const abandonFocus = (event: FocusEvent) => {
                if (event.target !== copyButtonRef.current && event.target !== document.body) {
                    attempt.abandoned = true;
                }
            };
            const abandonTab = (event: KeyboardEvent) => {
                if (event.key === "Tab") attempt.abandoned = true;
            };
            document.addEventListener("pointerdown", abandonPointer, true);
            document.addEventListener("focusin", abandonFocus, true);
            document.addEventListener("keydown", abandonTab, true);
            return () => {
                document.removeEventListener("pointerdown", abandonPointer, true);
                document.removeEventListener("focusin", abandonFocus, true);
                document.removeEventListener("keydown", abandonTab, true);
            };
        }
        if (!attempt.sawPending) return;
        copyFocusRef.current = null;
        const button = copyButtonRef.current;
        // Native disabling drops a focused Copy button onto body.
        if (
            attempt.ownedFocus && !attempt.abandoned && button?.isConnected && !button.disabled &&
            (document.activeElement === document.body || document.activeElement === button)
        ) {
            button.focus();
        }
    }, [copyPending, item?.id]);
    const availability = useClipboardWriteAvailability(item?.content_type ?? null);
    const copyAvailability = clipboardCopyPresentation(
        availability.isPending
            ? { status: "loading" }
            : availability.isError
              ? { status: "failed" }
              : { status: "resolved", availability: availability.data },
    );
    const revealed = revealedContent !== null;
    const close = () => {
        if (copyPending) return;
        if (revealed) onHide();
        onClose();
    };

    if (!item) {
        return (
            <InspectorShell
                className={styles.inspector}
                aria-label={t("history.inspector.label")}
                title={t("history.inspector.label")}
                headerActions={
                    <ActionButton
                        size="compactIcon"
                        icon="close"
                        disabled={copyPending}
                        aria-label={t("common.close")}
                        onClick={close}
                    />
                }
            >
                <div className={styles.empty}>
                    <strong>{t("history.inspector.emptyTitle")}</strong>
                    <p>{t("history.inspector.emptyBody")}</p>
                </div>
            </InspectorShell>
        );
    }

    const kind = kindOf(item);
    const potentialFinding = !item.is_sensitive ? item.sensitive_finding : null;
    const potentialRevealed =
        potentialFinding !== null &&
        shownFinding?.id === item.id &&
        shownFinding.finding === potentialFinding;
    const body = resolveClipBodyPresentation({
        item,
        fullContent,
        fullContentFailed,
        revealedContent,
        showPotentialSensitiveOriginal: potentialRevealed,
    });
    const source = clipSourceMetadata(item);
    const content = body.state === "content" ? body.content : "";
    const type = clipTypeMetadata(kind, content || item.content || "");
    const copyAction = clipCopyAction(kind);
    const SourceIcon = iconComponent(source.icon);
    const device = origin ? originName(origin) : t("common.unknown");
    const created = absoluteTime(item.created_at);

    return (
        <InspectorShell
            className={styles.inspector}
            aria-label={t("history.inspector.label")}
            title={t("history.inspector.label")}
            headerActions={
                <ActionButton
                    size="compactIcon"
                    icon="close"
                    disabled={copyPending}
                    aria-label={t("common.close")}
                    title={t("common.close")}
                    onClick={close}
                />
            }
            actions={
                <>
                    <ActionButton
                        size="compactIcon"
                        icon="expand"
                        disabled={copyPending}
                        aria-label={t("history.row.open")}
                        title={t("history.row.open")}
                        onClick={(event) => {
                            setShownFinding(null);
                            onOpenReader(item, event.currentTarget);
                        }}
                    />
                    <ActionButton
                        ref={copyButtonRef}
                        size="compactIcon"
                        variant="primary"
                        disabled={copyPending || !copyAvailability.canCopy}
                        icon={copyAction.icon}
                        aria-label={copyAction.label}
                        title={copyAction.label}
                        onClick={(event) => {
                            if (!copyFocusRef.current) {
                                const attempt = {
                                    itemId: item.id,
                                    ownedFocus: document.activeElement === event.currentTarget,
                                    sawPending: false,
                                    abandoned: false,
                                };
                                copyFocusRef.current = attempt;
                                // A declined copy has no pending render to clear its focus attempt.
                                window.setTimeout(() => {
                                    if (copyFocusRef.current === attempt && !attempt.sawPending) {
                                        copyFocusRef.current = null;
                                    }
                                }, 0);
                            }
                            onCopy(item);
                        }}
                    />
                    <ActionButton
                        size="compactIcon"
                        icon={item.pinned ? "unpin" : "pin"}
                        disabled={copyPending}
                        aria-pressed={item.pinned}
                        aria-label={t(
                            item.pinned
                                ? "history.row.unpin"
                                : "history.row.pin",
                        )}
                        title={t(
                            item.pinned
                                ? "history.row.unpin"
                                : "history.row.pin",
                        )}
                        onClick={() => onTogglePin(item)}
                    />
                    <ActionButton
                        size="compactIcon"
                        tone="danger"
                        disabled={copyPending}
                        icon="trash"
                        aria-label={t("history.row.delete")}
                        title={t("history.row.delete")}
                        onClick={() => onDelete(item)}
                    />
                    {potentialFinding !== null ? (
                        <Button
                            variant="secondary"
                            disabled={copyPending}
                            aria-pressed={potentialRevealed}
                            onClick={() =>
                                setShownFinding(
                                    potentialRevealed
                                        ? null
                                        : { id: item.id, finding: potentialFinding },
                                )
                            }
                        >
                            <Icon name={potentialRevealed ? "eyeOff" : "eye"} />
                            {t(
                                potentialRevealed
                                    ? "history.row.hideOriginal"
                                    : "history.row.showOriginal",
                            )}
                        </Button>
                    ) : null}
                    {revealed ? (
                        <Button variant="secondary" disabled={copyPending} onClick={onHide}>
                            <Icon name="eyeOff" />
                            {t("history.detail.hide")}
                        </Button>
                    ) : null}
                </>
            }
            metadata={
                <MetadataList density="compact">
                    {source.available ? (
                        <MetadataRow>
                            <MetadataLabel>
                                {t("history.inspector.application")}
                            </MetadataLabel>
                            <MetadataValue
                                className={styles.applicationValue}
                            >
                                <SourceAppIcon
                                    bundleId={item.source_app_bundle_id}
                                    Fallback={SourceIcon}
                                    fallbackText={source.label.slice(0, 2)}
                                    size="xs"
                                />
                                <TruncatedValue value={source.label} />
                            </MetadataValue>
                        </MetadataRow>
                    ) : null}
                    <MetadataRow>
                        <MetadataLabel>
                            {t("history.inspector.created")}
                        </MetadataLabel>
                        <MetadataValue>
                            <TruncatedValue value={created} />
                        </MetadataValue>
                    </MetadataRow>
                    <MetadataRow>
                        <MetadataLabel>
                            {t("history.inspector.device")}
                        </MetadataLabel>
                        <MetadataValue>
                            <DeviceMeta
                                label={device}
                                kind={origin?.kind ?? "unknown"}
                            />
                        </MetadataValue>
                    </MetadataRow>
                    <MetadataRow>
                        <MetadataLabel>
                            {t("history.inspector.type")}
                        </MetadataLabel>
                        <MetadataValue>
                            <TruncatedValue value={type.label} />
                        </MetadataValue>
                    </MetadataRow>
                    {kind !== "image" && body.state === "content" ? (
                        <MetadataRow>
                            <MetadataLabel>
                                {t("history.inspector.characters")}
                            </MetadataLabel>
                            <MetadataValue>
                                {Array.from(content).length.toLocaleString()}
                            </MetadataValue>
                        </MetadataRow>
                    ) : null}
                    <MetadataRow>
                        <MetadataLabel>
                            {t("history.inspector.savedAs")}
                        </MetadataLabel>
                        <MetadataValue>
                            {t(
                                item.pinned
                                    ? "history.inspector.pinned"
                                    : "history.inspector.historyItem",
                            )}
                        </MetadataValue>
                    </MetadataRow>
                    <MetadataRow>
                        <MetadataLabel>
                            {t("history.inspector.cloudEligibility")}
                        </MetadataLabel>
                        <MetadataValue>
                            {t(
                                item.too_large_to_sync
                                    ? "history.inspector.tooLarge"
                                    : "history.inspector.eligible",
                            )}
                        </MetadataValue>
                    </MetadataRow>
                </MetadataList>
            }
        >
            {copyAvailability.reason !== null ? (
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
            ) : null}
            {potentialFinding !== null ? (
                <InlineNotice role="status" tone="warning" icon="sensitive">
                    {t("history.row.potentialSensitiveWarning")}
                </InlineNotice>
            ) : null}
            <PreviewSurface
                className={styles.preview}
                elevation="flat"
                border="subtle"
                radius="md"
                padding="compact"
            >
                <div className={styles.previewLayout}>
                    <div className={styles.source}>
                        {source.available ? (
                            <SourceAppIcon
                                bundleId={item.source_app_bundle_id}
                                Fallback={SourceIcon}
                                fallbackText={source.label.slice(0, 2)}
                            />
                        ) : null}
                        <span className={styles.sourceCopy}>
                            {source.available ? (
                                <strong title={source.label}>
                                    {source.label}
                                </strong>
                            ) : null}
                            <small title={created}>{created}</small>
                        </span>
                        <span
                            className={styles.type}
                            title={type.label}
                            aria-label={type.label}
                        >
                            <Icon name={type.icon} size="sm" />
                        </span>
                    </div>
                    <div className={styles.previewBody}>
                        <div className={styles.previewContent}>
                            {body.state === "masked" ? (
                                <Button
                                    type="button"
                                    variant="ghost"
                                    disabled={copyPending}
                                    className={styles.protected}
                                    aria-label={t(
                                        "history.row.sensitiveReveal",
                                    )}
                                    aria-busy={revealPending || undefined}
                                    onClick={() =>
                                        !revealPending && onReveal(item)
                                    }
                                >
                                    <span
                                        aria-hidden="true"
                                        className={styles.protectedLines}
                                    >
                                        <i />
                                        <i />
                                        <i />
                                    </span>
                                    <Icon name="eye" size="sm" />
                                    <strong>
                                        {t(
                                            "history.row.sensitivePlaceholder",
                                        )}
                                    </strong>
                                </Button>
                            ) : body.state === "unavailable" ? (
                                <div role="status" className={styles.unavailable}>
                                    {t("history.detail.fullBodyUnavailable")}
                                </div>
                            ) : body.source === "preview" ? (
                                <div role="status" className={styles.unavailable}>
                                    {t("history.empty.loading.title")}
                                </div>
                            ) : (
                                <InspectorPreview
                                    kind={kind}
                                    ariaLabel={t("history.detail.contents")}
                                    content={
                                        content === ""
                                            ? t("history.detail.empty")
                                            : content
                                    }
                                    imagePreview={
                                        kind === "image" ? (
                                            <ClipImageLoader
                                                id={item.id}
                                                size="fill"
                                                loadingLabel={t(
                                                    "history.detail.imageLoading",
                                                )}
                                                failureLabel={t(
                                                    "history.detail.imageUnavailable",
                                                )}
                                            />
                                        ) : undefined
                                    }
                                />
                            )}
                        </div>
                    </div>
                </div>
            </PreviewSurface>
        </InspectorShell>
    );
}
