import { StateView } from "@/components/shared/StateView";
import { Button, Icon } from "@/components/ui";
import { useTranslation } from "@/i18n";
import { useServiceSettings } from "./ServiceSettingsController";

export function ServiceRestartNotice() {
  const { t } = useTranslation();
  const controller = useServiceSettings();
  if (!controller.restartRequired) return null;

  return (
    <StateView
      role="status"
      mode="warning"
      placement="inline"
      icon="refresh"
      actions={
        <Button
          variant="secondary"
          size="sm"
          disabled={controller.restartPending}
          aria-busy={controller.restartPending || undefined}
          onClick={controller.restart}
        >
          <Icon name="refresh" aria-hidden="true" />
          {t(
            controller.restartPending
              ? "settings.service.liveness.restarting"
              : "settings.service.liveness.restart",
          )}
        </Button>
      }
      title={t("settings.service.liveness.pending")}
    />
  );
}
