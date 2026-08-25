import { Icon, type IconName } from "@/components/ui/icon";
import { Button } from "@/components/ui";
import {
  useOnboardingPermissions,
  usePermissionRequest,
} from "@/hooks/useOnboardingPermissions";
import { useCaptureMutation, useCaptureNow, useCaptureState } from "@/hooks/useCapture";
import { useSetServiceConfig } from "@/hooks/useServiceConfig";
import { useTranslation } from "@/i18n";
import { captureArm } from "@/lib/ipc";
import styles from "./AndroidCaptureSetup.module.css";


export function AndroidCaptureSetup() {
  const { t } = useTranslation();
  const permissions = useOnboardingPermissions();
  const request = usePermissionRequest();
  const save = useSetServiceConfig();
  const capture = useCaptureState();
  const now = useCaptureNow();
  const arm = useCaptureMutation();
  const notificationStatus = permissions.data?.notifications.status;
  const tileStatus = permissions.data?.tile.status;
  const captureWorking = capture.data?.health.state === "working";
  const busy = request.isPending || save.isPending || now.isPending || arm.isPending;

  const askNotifications = () => {
    request.mutate("notifications", {
      onSuccess: (fresh) => {
        if (["granted", "not_required"].includes(fresh.notifications.status)) {
          save.mutate({ notify_on_copy: true });
        }
      },
    });
  };

  return (
    <section className={styles.root} aria-label={t("onboarding.capture.androidSetupLabel")}>
      <SetupAction
        icon="library"
        title={t("onboarding.capture.saveNow")}
        detail={t("onboarding.capture.saveNowDetail")}
        label={t("onboarding.capture.saveNowAction")}
        disabled={busy}
        onClick={() => now.mutate("in_app")}
      />
      <SetupAction
        icon="devices"
        title={t("onboarding.capture.addTile")}
        detail={t("onboarding.capture.addTileDetail")}
        label={tileStatus === "granted"
          ? t("onboarding.capture.tileAdded")
          : t("onboarding.capture.addTileAction")}
        disabled={busy || tileStatus === "granted" || tileStatus === "unavailable"}
        onClick={() => request.mutate("tile")}
      />
      <SetupAction
        icon="alert"
        title={t("onboarding.capture.notifications")}
        detail={t("onboarding.capture.notificationsDetail")}
        label={notificationStatus === "granted"
          ? t("onboarding.capture.notificationsAllowed")
          : t("onboarding.capture.notificationsAction")}
        disabled={busy || notificationStatus === "granted"}
        onClick={askNotifications}
      />
      <SetupAction
        icon="play"
        title={t("onboarding.capture.background")}
        detail={t("onboarding.capture.backgroundDetail")}
        label={captureWorking
          ? t("onboarding.capture.backgroundActive")
          : t("onboarding.capture.backgroundAction")}
        disabled={busy || captureWorking || capture.data === undefined}
        onClick={() => arm.mutate(() => captureArm())}
      />
    </section>
  );
}

function SetupAction({
  icon,
  title,
  detail,
  label,
  disabled,
  onClick,
}: {
  icon: IconName;
  title: string;
  detail: string;
  label: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <div className={styles.action}>
      <span className={styles.icon} aria-hidden="true"><Icon name={icon} size="md" /></span>
      <span className={styles.copy}>
        <strong>{title}</strong>
        <small>{detail}</small>
      </span>
      <Button type="button" variant="secondary" size="sm" disabled={disabled} onClick={onClick}>
        {label}
      </Button>
    </div>
  );
}
