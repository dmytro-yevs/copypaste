import { Icon, type IconName } from "@/components/ui/icon";
import { Button } from "@/components/ui";
import {
  useOnboardingPermissions,
  usePermissionOpenSettings,
  usePermissionRequest,
} from "@/hooks/useOnboardingPermissions";
import {
  useCaptureMutation,
  useCaptureNow,
  useCaptureState,
} from "@/hooks/useCapture";
import { useSetServiceConfig } from "@/hooks/useServiceConfig";
import {
  permissionPresentation,
  type PermissionAction,
  type PermissionExplanation,
  type PermissionLabel,
} from "@/features/onboarding/model/permissionPresentation";
import { useTranslation } from "@/i18n";
import {
  captureArm,
  type OnboardingPermissionId,
  type OnboardingPermissions,
  type OnboardingPermissionStatus,
} from "@/lib/ipc";
import styles from "./AndroidCaptureSetup.module.css";


export function AndroidCaptureSetup() {
  const { t } = useTranslation();
  const permissions = useOnboardingPermissions();
  const request = usePermissionRequest();
  const openSettings = usePermissionOpenSettings();
  const save = useSetServiceConfig();
  const capture = useCaptureState();
  const now = useCaptureNow();
  const arm = useCaptureMutation();
  const notificationStatus = permissions.data?.notifications.status;
  const tileStatus = permissions.data?.tile.status;
  const permissionReadFailed = permissions.error !== null;
  const captureWorking = capture.data?.health.state === "working";
  const busy =
    permissions.isPending ||
    request.isPending ||
    openSettings.isPending ||
    save.isPending ||
    now.isPending ||
    arm.isPending;

  const afterPermission = (id: OnboardingPermissionId) => ({
    onSuccess: (fresh: OnboardingPermissions) => {
      if (
        id === "notifications" &&
        (fresh.notifications.status === "granted" ||
          fresh.notifications.status === "not_required")
      ) {
        save.mutate({ notify_on_copy: true });
      }
    },
  });

  const runPermission = (
    id: OnboardingPermissionId,
    action: PermissionAction,
  ) => {
    if (action === "request") request.mutate(id, afterPermission(id));
    if (action === "open-settings") {
      openSettings.mutate(id, afterPermission(id));
    }
  };

  return (
    <section
      className={styles.root}
      aria-label={t("onboarding.capture.androidSetupLabel")}
    >
      <SetupAction
        icon="library"
        title={t("onboarding.capture.saveNow")}
        detail={t("onboarding.capture.saveNowDetail")}
        label={t("onboarding.capture.saveNowAction")}
        disabled={busy}
        onClick={() => now.mutate("in_app")}
      />
      <PermissionSetupAction
        id="tile"
        icon="devices"
        title={t("onboarding.capture.addTile")}
        defaultDetail={t("onboarding.capture.addTileDetail")}
        status={permissionReadFailed ? "unavailable" : tileStatus}
        busy={busy}
        onRun={runPermission}
      />
      <PermissionSetupAction
        id="notifications"
        icon="alert"
        title={t("onboarding.capture.notifications")}
        defaultDetail={t("onboarding.capture.notificationsDetail")}
        status={permissionReadFailed ? "unavailable" : notificationStatus}
        busy={busy}
        onRun={runPermission}
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

function PermissionSetupAction({
  id,
  icon,
  title,
  defaultDetail,
  status,
  busy,
  onRun,
}: {
  id: OnboardingPermissionId;
  icon: IconName;
  title: string;
  defaultDetail: string;
  status?: OnboardingPermissionStatus;
  busy: boolean;
  onRun: (id: OnboardingPermissionId, action: PermissionAction) => void;
}) {
  const { t } = useTranslation();
  const presentation = permissionPresentation(status ?? "prompt");

  const label = t(permissionLabelKey(id, presentation.label));
  const detailKey = permissionDetailKey(presentation.explanation);
  const detail = detailKey === null ? defaultDetail : t(detailKey);

  return (
    <SetupAction
      icon={icon}
      title={title}
      detail={detail}
      label={label}
      disabled={busy || status === undefined || presentation.disabled}
      onClick={() => onRun(id, presentation.action)}
    />
  );
}

function permissionLabelKey(
  id: OnboardingPermissionId,
  label: PermissionLabel,
) {
  switch (label) {
    case "request":
      return id === "tile"
        ? "onboarding.capture.addTileAction"
        : "onboarding.capture.notificationsAction";
    case "granted":
      return id === "tile"
        ? "onboarding.capture.tileAdded"
        : "onboarding.capture.notificationsAllowed";
    case "open-settings":
      return "onboarding.capture.permission.openSettings";
    case "not-required":
      return "onboarding.capture.permission.notRequired";
    case "unavailable":
      return "onboarding.capture.permission.unavailable";
  }
}

function permissionDetailKey(
  explanation: PermissionExplanation,
) {
  switch (explanation) {
    case "default":
      return null;
    case "denied":
      return "onboarding.capture.permission.deniedDetail";
    case "not-required":
      return "onboarding.capture.permission.notRequiredDetail";
    case "unavailable":
      return "onboarding.capture.permission.unavailableDetail";
  }
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
