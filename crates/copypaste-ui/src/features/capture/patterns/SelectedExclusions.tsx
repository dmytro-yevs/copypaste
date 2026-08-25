import { ActionButton } from "@/components/shared";
import { iconComponent } from "@/components/ui";
import { SourceAppIcon } from "@/features/source-apps";
import { clipSourceMetadata } from "@/lib/clipSourcePresentation";
import type { InstalledSourceApp, Item } from "@/lib/ipc";
import { useTranslation } from "@/i18n";

import styles from "./SourceExclusions.module.css";

interface SelectedExclusionsProps {
    ids: readonly string[];
    installedById: ReadonlyMap<string, InstalledSourceApp>;
    firstByApp: ReadonlyMap<string, Item>;
    disabled: boolean;
    onChange: (ids: string[]) => void;
}

export function SelectedExclusions({
    ids,
    installedById,
    firstByApp,
    disabled,
    onChange,
}: SelectedExclusionsProps) {
    const { t } = useTranslation();

    if (ids.length === 0) return null;

    return (
        <ul className={styles.selectedList}>
            {ids.map((id) => {
                const app = installedById.get(id);
                const item = firstByApp.get(id);
                const source = item ? clipSourceMetadata(item) : null;
                const label = app?.label ?? source?.label;
                return (
                    <li key={id} className={styles.selectedItem}>
                        <SourceAppIcon
                            bundleId={id}
                            Fallback={
                                source
                                    ? iconComponent(source.icon)
                                    : iconComponent("search")
                            }
                            size="xs"
                        />
                        <span className={styles.appCopy}>
                            {label && (
                                <span className={styles.appLabel} title={label}>
                                    {label}
                                </span>
                            )}
                            <span className={styles.appId} title={id}>
                                {id}
                            </span>
                        </span>
                        <ActionButton
                            type="button"
                            variant="ghost"
                            size="icon"
                            icon="trash"
                            aria-label={t(
                                "settings.service.exclusions.remove",
                                { id },
                            )}
                            disabled={disabled}
                            onClick={() =>
                                onChange(
                                    ids.filter((candidate) => candidate !== id),
                                )
                            }
                        />
                    </li>
                );
            })}
        </ul>
    );
}
