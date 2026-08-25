import { ActionButton } from "@/components/shared";
import { Checkbox } from "@/components/ui";
import { useTranslation } from "@/i18n";
import styles from "./BulkActionBar.module.css";

interface BulkActionBarProps {
    count: number;
    total: number;
    allSelected: boolean;
    allPinned: boolean;
    busy: boolean;
    onToggleAll: () => void;
    onSelectAll: () => void;
    onTogglePin: () => void;
    onDelete: () => void;
    onClose: () => void;
}

export function BulkActionBar({
    count,
    total,
    allSelected,
    allPinned,
    busy,
    onToggleAll,
    onSelectAll,
    onTogglePin,
    onDelete,
    onClose,
}: BulkActionBarProps) {
    const { t } = useTranslation();
    const selectedLabel = t("history.bulk.selected", { count });

    return (
        <div className={styles.root}>
            <div className={styles.summary}>
                <Checkbox
                    checked={allSelected ? true : "indeterminate"}
                    disabled={busy}
                    aria-label={
                        allSelected
                            ? t("history.bulk.clear")
                            : t("history.bulk.selectAll")
                    }
                    onCheckedChange={onToggleAll}
                />
                <strong aria-live="polite">{selectedLabel}</strong>
            </div>

            <div className={styles.actions}>
                <div className={styles.actionsLayout}>
                    {!allSelected && count < total ? (
                        <ActionButton
                            size="compactIcon"
                            icon="selectAll"
                            aria-label={t("history.bulk.selectAll")}
                            disabled={busy}
                            onClick={onSelectAll}
                        />
                    ) : null}
                    <ActionButton
                        size="compactIcon"
                        icon={allPinned ? "unpin" : "pin"}
                        aria-label={t(
                            allPinned ? "history.bulk.unpin" : "history.bulk.pin",
                        )}
                        disabled={busy}
                        onClick={onTogglePin}
                    />
                    <ActionButton
                        size="compactIcon"
                        tone="danger"
                        icon="trash"
                        aria-label={t("history.bulk.delete")}
                        disabled={busy}
                        onClick={onDelete}
                    />
                    <ActionButton
                        size="compactIcon"
                        icon="close"
                        aria-label={t("history.bulk.done")}
                        disabled={busy}
                        onClick={onClose}
                    />
                </div>
            </div>
        </div>
    );
}
