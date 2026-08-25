
import {
  PaneHeader,
  PaneHeaderCopy,
  Screen,
  ScrollViewport,
} from "@/components/layout";
import { CaptureSetupState } from "@/features/capture/patterns/CaptureSetup";
import { useTranslation } from "@/i18n";
import styles from "./CaptureScreen.module.css";

export function CaptureScreen() {
  const { t } = useTranslation();

  return (
    <Screen className={styles.root}>
      <PaneHeader className={styles.header}>
        <PaneHeaderCopy>
          <h1 className={styles.title}>{t("capture.title")}</h1>
        </PaneHeaderCopy>
      </PaneHeader>

      <ScrollViewport padding="compact" className={styles.viewport}>
        <CaptureSetupState />
      </ScrollViewport>
    </Screen>
  );
}
