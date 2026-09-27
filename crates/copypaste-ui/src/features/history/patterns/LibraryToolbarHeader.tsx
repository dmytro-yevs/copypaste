import { Container } from "@/components/layout";
import { ScreenHeader } from "@/components/shared";
import {
    historyCompactCount,
    historyCount,
} from "@/features/history/model/libraryToolbarOptions";
import { useTranslation } from "@/i18n";
import styles from "./LibraryToolbar.module.css";

interface LibraryToolbarHeaderProps {
    compact: boolean;
    filtered: boolean;
    visible: number;
    total: number | undefined;
}

export function LibraryToolbarHeader({
    compact,
    filtered,
    visible,
    total,
}: LibraryToolbarHeaderProps) {
    const { t } = useTranslation();
    return (
        <Container width="library" gutter="screen" className={styles.header}>
            <ScreenHeader
                title={t("history.header.title")}
                actions={
                    compact ? (
                        <output
                            className={styles.headerCount}
                            data-slot="history-count"
                            aria-label={historyCount(filtered, visible, total)}
                        >
                            <span aria-hidden="true">
                                {historyCompactCount(filtered, visible, total)}
                            </span>
                        </output>
                    ) : undefined
                }
            />
        </Container>
    );
}
