import {
    ActionButton,
    DeviceMeta,
    InspectorShell,
    MetadataLabel,
    MetadataList,
    MetadataRow,
    MetadataValue,
    PreviewSurface,
    TruncatedValue,
} from "@/components/shared";
import { ClipImageLoader } from "@/features/clip-content";
import { Button, Icon, iconComponent } from "@/components/ui";
import { InspectorPreview } from "@/features/history/components/InspectorPreview";
import { clipCopyAction } from "@/features/history/model/clipPresentation";
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
    onReveal: (item: Item) => void;
    onHide: () => void;
    onCopy: (item: Item) => void;
    onTogglePin: (item: Item) => void;
    onDelete: (item: Item) => void;
    onClose: () => void;
}

export function LibraryInspectorPanel({
    item,
    origin,
    revealedContent,
    fullContent,
    fullContentFailed,
    revealPending,
    onReveal,
    onHide,
    onCopy,
    onTogglePin,
    onDelete,
    onClose,
}: LibraryInspectorPanelProps) {
    const { t } = useTranslation();
    const revealed = revealedContent !== null;
    const close = () => {
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
    const body = resolveClipBodyPresentation({
        item,
        fullContent,
        fullContentFailed,
        revealedContent,
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
                    aria-label={t("common.close")}
                    title={t("common.close")}
                    onClick={close}
                />
            }
            actions={
                <>
                    <ActionButton
                        size="compactIcon"
                        variant="primary"
                        icon={copyAction.icon}
                        aria-label={copyAction.label}
                        title={copyAction.label}
                        onClick={() => onCopy(item)}
                    />
                    <ActionButton
                        size="compactIcon"
                        icon={item.pinned ? "unpin" : "pin"}
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
                        icon="trash"
                        aria-label={t("history.row.delete")}
                        title={t("history.row.delete")}
                        onClick={() => onDelete(item)}
                    />
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
