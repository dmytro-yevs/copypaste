import { Icon } from "@/components/ui/icon";
import { Button } from "@/components/ui";
import type { CloudAccountController } from "@/features/settings/hooks/useCloudAccountController";
import { useTranslation } from "@/i18n";
import styles from "../CloudSyncSettings.module.css";

export function CloudConnectedControls({
  controller,
  descriptionId,
}: {
  controller: CloudAccountController;
  descriptionId: string;
}) {
  const { t } = useTranslation();

  return (
    <div className={styles.connected}>
      {controller.status?.email ? (
        <span className={styles.email}>{controller.status.email}</span>
      ) : null}
      <div className={styles.actions}>
        <Button
          variant="secondary"
          size="sm"
          disabled={controller.busy}
          state={controller.syncPending ? "loading" : "normal"}
          aria-describedby={descriptionId}
          onClick={controller.syncNow}
        >
          {!controller.syncPending ? <Icon name="refresh" aria-hidden="true" /> : null}
          {t(controller.syncPending
            ? "settings.sync.cloud.syncing"
            : "settings.sync.cloud.syncNow")}
        </Button>
        <Button
          variant="secondary"
          size="sm"
          disabled={controller.busy}
          state={controller.signOutPending ? "loading" : "normal"}
          aria-describedby={descriptionId}
          onClick={controller.submitSignOut}
        >
          {!controller.signOutPending ? <Icon name="logout" aria-hidden="true" /> : null}
          {t(controller.signOutPending
            ? "settings.sync.cloud.signingOut"
            : "settings.sync.cloud.signOut")}
        </Button>
      </div>
    </div>
  );
}
