import type { ReactNode } from "react";
import { ErrorBoundary } from "react-error-boundary";

import { Container, Screen } from "@/components/layout";
import { IllustratedErrorState, ScreenHeader } from "@/components/shared";
import { Button, Icon } from "@/components/ui";
import { useTranslation } from "@/i18n";
import { useUi } from "@/store/ui";
import styles from "./Boundary.module.css";

interface BoundaryProps {
  /** Named so a crash report says which region failed. */
  label: string;
  layout?: "inline" | "screen";
  onReset?: () => void;
  children: ReactNode;
}

export function Boundary({ label, layout = "inline", onReset, children }: BoundaryProps) {
  const { t } = useTranslation();
  const setView = useUi((state) => state.setView);
  const setSettingsTab = useUi((state) => state.setSettingsTab);

  return (
    <ErrorBoundary
      // Logged, never rendered: a stack contains a bundle path (INV-12).
      onError={(error) => console.error(`[copypaste] ${label} crashed`, error)}
      onReset={onReset}
      fallbackRender={({ resetErrorBoundary }) => {
        const message = (
          <IllustratedErrorState
            className={styles.recovery}
            title={t("shell.boundary.title", { region: label })}
            body={t("shell.boundary.body")}
            actions={
              <>
                <Button type="button" onClick={resetErrorBoundary}>
                  <Icon name="reset" size="sm" />
                  {t("shell.boundary.reload")}
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  onClick={() => {
                    setSettingsTab("diagnostics");
                    setView("settings");
                    resetErrorBoundary();
                  }}
                >
                  <Icon name="stethoscope" size="sm" />
                  {t("shell.boundary.diagnostics")}
                </Button>
              </>
            }
          />
        );

        return layout === "screen" ? (
          <Screen className={styles.screen}>
            <Container width="fluid" gutter="screen">
              <ScreenHeader title={label} />
              {message}
            </Container>
          </Screen>
        ) : (
          <div className={styles.inline}>{message}</div>
        );
      }}
    >
      {children}
    </ErrorBoundary>
  );
}
