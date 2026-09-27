import { Container } from "@/components/layout";
import { ScreenHeader } from "@/components/shared";
import { useTranslation } from "@/i18n";
import styles from "./LibraryToolbar.module.css";

export function LibraryToolbarHeader() {
    const { t } = useTranslation();
    return (
        <Container width="library" gutter="screen" className={styles.header}>
            <ScreenHeader title={t("history.header.title")} />
        </Container>
    );
}
