import { Icon } from "@/components/ui/icon";
import { Button } from "@/components/ui";
import { useTranslation } from "@/i18n";

import styles from "./SourceExclusions.module.css";

interface SourceExclusionsHeaderProps {
    collapsible: boolean;
    expanded: boolean;
    controlsId: string;
    android: boolean;
    windows: boolean;
    onToggle: () => void;
}

export function SourceExclusionsHeader({
    collapsible,
    expanded,
    controlsId,
    android,
    windows,
    onToggle,
}: SourceExclusionsHeaderProps) {
    const { t } = useTranslation();

    return (
        <div className={styles.headingGroup}>
            <h2 className={styles.heading}>
                {collapsible ? (
                    <Button
                        type="button"
                        variant="ghost"
                        aria-expanded={expanded}
                        aria-controls={controlsId}
                        className={styles.disclosure}
                        onClick={onToggle}
                    >
                        {t("settings.service.exclusions.title")}
                        <Icon
                            name="caretDown"
                            size="sm"
                            aria-hidden="true"
                            className={
                                expanded ? styles.disclosureExpanded : undefined
                            }
                        />
                    </Button>
                ) : (
                    t("settings.service.exclusions.title")
                )}
            </h2>
            <p className={styles.description}>
                {t(
                    android
                        ? "settings.service.exclusions.androidLimitation"
                        : windows
                          ? "settings.service.exclusions.windowsDescription"
                          : "settings.service.exclusions.description",
                )}
            </p>
        </div>
    );
}
