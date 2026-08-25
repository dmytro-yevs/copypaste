import {
    ActionButton,
    DefinitionList,
    DefinitionRow,
    DefinitionTerm,
    DefinitionValue,
    TruncatedValue,
    DeviceMeta,
    InspectorShell,
} from "@/components/shared";
import { Button, Icon, Surface, iconComponent } from "@/components/ui";
import { InspectorPreview } from "@/features/history/components/InspectorPreview";
import {
    clipSourceMetadata,
    clipTypeMetadata,
} from "@/features/history/model/clipPresentation";
import { originName, type OriginDevice } from "@/features/history/model/origin";
import { ClipImageLoader } from "@/features/history/patterns/ClipImageLoader";
import { SourceAppIcon } from "@/features/source-apps";
import { useTranslation } from "@/i18n";
import { absoluteTime, kindOf } from "@/lib/format";
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
    const source = clipSourceMetadata(item);
    const type = clipTypeMetadata(kind, fullContent ?? item.content ?? "");
    const SourceIcon = iconComponent(source.icon);
    const masked = item.is_sensitive && !revealed;
    const potential = item.is_sensitive ? null : item.sensitive_finding;
    const content =
        potential?.redacted_preview ??
        revealedContent ??
        fullContent ??
        item.content ??
        "";
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
                        icon="copy"
                        aria-label={t("history.detail.copy")}
                        title={t("history.detail.copy")}
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
                <DefinitionList>
                    {source.available ? (
                        <DefinitionRow>
                            <DefinitionTerm>
                                {t("history.inspector.application")}
                            </DefinitionTerm>
                            <DefinitionValue
                                className={styles.applicationValue}
                            >
                                <SourceAppIcon
                                    bundleId={item.source_app_bundle_id}
                                    Fallback={SourceIcon}
                                    fallbackText={source.label.slice(0, 2)}
                                    size="xs"
                                />
                                <TruncatedValue value={source.label} />
                            </DefinitionValue>
                        </DefinitionRow>
                    ) : null}
                    <DefinitionRow>
                        <DefinitionTerm>
                            {t("history.inspector.created")}
                        </DefinitionTerm>
                        <DefinitionValue>
                            <TruncatedValue value={created} />
                        </DefinitionValue>
                    </DefinitionRow>
                    <DefinitionRow>
                        <DefinitionTerm>
                            {t("history.inspector.device")}
                        </DefinitionTerm>
                        <DefinitionValue>
                            <DeviceMeta
                                label={device}
                                kind={origin?.kind ?? "unknown"}
                            />
                        </DefinitionValue>
                    </DefinitionRow>
                    <DefinitionRow>
                        <DefinitionTerm>
                            {t("history.inspector.type")}
                        </DefinitionTerm>
                        <DefinitionValue>
                            <TruncatedValue value={type.label} />
                        </DefinitionValue>
                    </DefinitionRow>
                    {kind !== "image" && !masked ? (
                        <DefinitionRow>
                            <DefinitionTerm>
                                {t("history.inspector.characters")}
                            </DefinitionTerm>
                            <DefinitionValue>
                                {Array.from(content).length.toLocaleString()}
                            </DefinitionValue>
                        </DefinitionRow>
                    ) : null}
                    <DefinitionRow>
                        <DefinitionTerm>
                            {t("history.inspector.savedAs")}
                        </DefinitionTerm>
                        <DefinitionValue>
                            {t(
                                item.pinned
                                    ? "history.inspector.pinned"
                                    : "history.inspector.historyItem",
                            )}
                        </DefinitionValue>
                    </DefinitionRow>
                    <DefinitionRow>
                        <DefinitionTerm>
                            {t("history.inspector.cloudEligibility")}
                        </DefinitionTerm>
                        <DefinitionValue>
                            {t(
                                item.too_large_to_sync
                                    ? "history.inspector.tooLarge"
                                    : "history.inspector.eligible",
                            )}
                        </DefinitionValue>
                    </DefinitionRow>
                </DefinitionList>
            }
        >
            <Surface
                className={styles.preview}
                elevation="flat"
                border="subtle"
                radius="md"
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
                            {masked ? (
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
            </Surface>
        </InspectorShell>
    );
}
