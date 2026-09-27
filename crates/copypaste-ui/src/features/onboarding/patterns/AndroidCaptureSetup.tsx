import { useState, type ReactNode } from "react";

import { FieldFeedback } from "@/components/shared/FieldFeedback";
import { InlineNotice } from "@/components/shared/InlineNotice";
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
import {
  permissionPresentation,
  type PermissionAction,
  type PermissionExplanation,
  type PermissionLabel,
} from "@/features/onboarding/model/permissionPresentation";
import { useTranslation } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import {
  captureArm,
  type OnboardingPermissionId,
  type OnboardingPermissionStatus,
} from "@/lib/ipc";
import styles from "./AndroidCaptureSetup.module.css";


export function AndroidCaptureSetup() {
  const { t } = useTranslation();
  const permissions = useOnboardingPermissions();
  const request = usePermissionRequest();
  const openSettings = usePermissionOpenSettings();
  const capture = useCaptureState();
  const now = useCaptureNow();
  const arm = useCaptureMutation();
  const [actionState, setActionState] = useState<{
    id: OnboardingPermissionId;
    action: PermissionAction;
    error?: unknown;
  } | null>(null);
  const notificationStatus = permissions.data?.notifications.status;
  const tileStatus = permissions.data?.tile.status;
  const permissionReadFailed = permissions.error !== null;
  const captureWorking = capture.data?.health.state === "working";
  const busy =
    permissions.isFetching ||
    request.isPending ||
    openSettings.isPending ||
    now.isPending ||
    arm.isPending;

  const afterPermission = (id: OnboardingPermissionId, action: PermissionAction) => ({
    onSuccess: () => {
      setActionState(null);
    },
    onError: (error: unknown) => setActionState({ id, action, error }),
  });

  const runPermission = (
    id: OnboardingPermissionId,
    action: PermissionAction,
  ) => {
    setActionState({ id, action });
    if (action === "request") request.mutate(id, afterPermission(id, action));
    if (action === "open-settings") {
      openSettings.mutate(id, afterPermission(id, action));
    }
  };

  const permissionFeedback = (id: OnboardingPermissionId, action: PermissionAction) => {
    if (actionState?.id !== id || actionState.action !== action) return null;
    if (request.isPending || openSettings.isPending) {
      return <FieldFeedback state="pending">{t("onboarding.capture.permission.actionPending")}</FieldFeedback>;
    }
    return "error" in actionState ? (
      <FieldFeedback state="error">
        {t("onboarding.capture.permission.actionFailed", { error: toFriendly(actionState.error) })}
      </FieldFeedback>
    ) : null;
  };

  return (
    <section
      className={styles.root}
      aria-label={t("onboarding.capture.androidSetupLabel")}
    >
      {permissionReadFailed ? (
        <InlineNotice
          role="alert"
          tone="warning"
          icon="alert"
          action={
            <Button type="button" variant="secondary" size="sm" disabled={busy} onClick={() => void permissions.refetch()}>
              {t("onboarding.capture.permission.retryCheck")}
            </Button>
          }
        >
          {t("onboarding.capture.permission.checkFailedDetail")}
        </InlineNotice>
      ) : null}
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
        readFailed={permissionReadFailed}
        busy={busy}
        onRun={runPermission}
        feedback={permissionFeedback("tile", permissionPresentation(tileStatus ?? "prompt").action)}
      />
      <PermissionSetupAction
        id="notifications"
        icon="alert"
        title={t("onboarding.capture.notifications")}
        defaultDetail={t("onboarding.capture.notificationsDetail")}
        status={permissionReadFailed ? "unavailable" : notificationStatus}
        readFailed={permissionReadFailed}
        busy={busy}
        onRun={runPermission}
        feedback={permissionFeedback("notifications", permissionPresentation(notificationStatus ?? "prompt").action)}
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
  readFailed,
  busy,
  onRun,
  feedback,
}: {
  id: OnboardingPermissionId;
  icon: IconName;
  title: string;
  defaultDetail: string;
  status?: OnboardingPermissionStatus;
  readFailed: boolean;
  busy: boolean;
  onRun: (id: OnboardingPermissionId, action: PermissionAction) => void;
  feedback: ReactNode;
}) {
  const { t } = useTranslation();
  const presentation = permissionPresentation(status ?? "prompt");

  const label = readFailed ? t("onboarding.capture.permission.checkFailed") : t(permissionLabelKey(id, presentation.label));
  const detailKey = permissionDetailKey(presentation.explanation);
  const detail = readFailed || detailKey === null ? defaultDetail : t(detailKey);

  return (
    <SetupAction
      icon={icon}
      title={title}
      detail={detail}
      feedback={readFailed ? null : feedback}
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
  feedback,
  label,
  disabled,
  onClick,
}: {
  icon: IconName;
  title: string;
  detail: string;
  feedback?: ReactNode;
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
        {feedback}
      </span>
      <Button type="button" variant="secondary" size="sm" disabled={disabled} onClick={onClick}>
        {label}
      </Button>
    </div>
  );
}
