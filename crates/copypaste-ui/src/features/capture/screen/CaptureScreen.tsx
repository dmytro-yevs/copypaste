import { useCallback } from "react";
import {
  Container,
  Screen,
  ScrollViewport,
} from "@/components/layout";
import { ActionButton, ScreenHeader } from "@/components/shared";
import { CaptureSetupState } from "@/features/capture/patterns/CaptureSetup";
import { useSettingsLevel } from "@/features/settings/hooks/useSettingsLevel";
import { useTranslation } from "@/i18n";
import { useUi } from "@/store/ui";
import styles from "./CaptureScreen.module.css";

export function CaptureScreen() {
  const { t } = useTranslation();
  const close = useCallback((path: readonly string[]) => {
    if (path.length === 0) useUi.getState().setView("history");
  }, []);
  const back = useSettingsLevel(["capture"], close);

  return (
    <Screen className={styles.root}>
      <ScreenHeader
        className={styles.header}
        leading={<ActionButton
          variant="ghost"
          size="compactIcon"
          icon="back"
          aria-label={t("capture.back")}
          onClick={back}
        />}
        title={t("capture.title")}
      />

      <ScrollViewport className={styles.viewport}>
        <Container width="reading" gutter="compact" className={styles.content}>
          <CaptureSetupState />
        </Container>
      </ScrollViewport>
    </Screen>
  );
}
