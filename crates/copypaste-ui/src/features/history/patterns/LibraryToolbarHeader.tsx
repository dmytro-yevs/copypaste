import { Container } from "@/components/layout";
import { ScreenHeader } from "@/components/shared";
import { historyCountNumber } from "@/features/history/model/libraryToolbarOptions";
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
                eyebrow={t("history.header.eyebrow")}
                title={t("history.header.title")}
                description={t("history.header.description")}
                actions={
                    compact ? (
                        <span aria-hidden="true" className={styles.headerCount}>
                            {historyCountNumber(filtered, visible, total)}
                        </span>
                    ) : undefined
                }
            />
        </Container>
    );
}
