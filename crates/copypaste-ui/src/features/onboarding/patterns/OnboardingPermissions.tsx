import { useId, type ReactNode } from "react";

import { StateView } from "@/components/shared/StateView";
import { Button, Icon, Switch, type IconName } from "@/components/ui";
import { permissionPresentation } from "@/features/onboarding/model/permissionPresentation";
import { useOnboardingPermissions, usePermissionOpenSettings, usePermissionRequest } from "@/hooks/useOnboardingPermissions";
import { useOpenAtLogin, useSetOpenAtLogin } from "@/hooks/useOpenAtLogin";
import { useTranslation } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import type { OnboardingPermissionItem } from "@/lib/ipc";
import styles from "./OnboardingPermissions.module.css";

export function OnboardingPermissions({ android }: { android: boolean }) {
  const { t } = useTranslation();
  const permissions = useOnboardingPermissions();
  const request = usePermissionRequest();
  const settings = usePermissionOpenSettings();
  const busy = permissions.isFetching || request.isPending || settings.isPending;
  const error = request.error ?? settings.error;
  const run = (item: OnboardingPermissionItem) => {
    request.reset();
    settings.reset();
    if (permissionPresentation(item.status).action === "open-settings") settings.mutate(item.id);
    else request.mutate(item.id);
  };
  return (
    <div className={styles.root}>
      <div className={styles.rows}>
        <PermissionRow icon="alert" title={t("onboarding.permissions.notifications")} detail={t("onboarding.permissions.notificationsDetail")}
          item={permissions.data?.notifications} busy={busy} failed={permissions.isError} onRun={run} />
        {!android ? <StartupRow /> : null}
        <PermissionRow icon="battery" title={t("onboarding.permissions.background")} detail={t(android ? "onboarding.permissions.backgroundAndroid" : "onboarding.permissions.backgroundDesktop")}
          item={permissions.data?.backgroundActivity} busy={busy} failed={permissions.isError} onRun={run} />
      </div>
      {permissions.isError ? <StateView mode="error" placement="inline" title={t("onboarding.permissions.checkFailed")}
        actions={<Button variant="secondary" size="sm" onClick={() => void permissions.refetch()}>{t("common.tryAgain")}</Button>} /> : null}
      {error ? <StateView mode="error" placement="inline" title={toFriendly(error)} /> : null}
      <p className={styles.note}>{t("onboarding.permissions.optional")}</p>
    </div>
  );
}

function PermissionRow({ icon, title, detail, item, busy, failed, onRun }: {
  icon: IconName; title: string; detail: string; item?: OnboardingPermissionItem;
  busy: boolean; failed: boolean; onRun: (item: OnboardingPermissionItem) => void;
}) {
  const { t } = useTranslation();
  const presentation = item ? permissionPresentation(item.status) : null;
  const complete = !failed && (item?.status === "granted" || item?.status === "not_required");
  const label = failed ? "checkFailed" : presentation?.label ?? "checking";
  return (
    <PermissionLayout icon={icon} title={title} detail={detail}>
      {complete ? <span className={styles.status}><Icon name="check" />{t(`onboarding.permissions.${label}`)}</span> : (
        <Button variant="secondary" size="sm" aria-label={`${title}: ${t(`onboarding.permissions.${label}`)}`}
          disabled={busy || failed || !item || presentation?.disabled} onClick={() => item && onRun(item)}>
          {t(`onboarding.permissions.${label}`)}
        </Button>
      )}
    </PermissionLayout>
  );
}

function StartupRow() {
  const { t } = useTranslation();
  const titleId = useId();
  const startup = useOpenAtLogin();
  const save = useSetOpenAtLogin();
  return (
    <PermissionLayout icon="play" title={t("onboarding.startup.title")} detail={t("onboarding.startup.description")} titleId={titleId}>
      <Switch aria-labelledby={titleId} checked={startup.data ?? false} disabled={startup.isPending || startup.isError || save.isPending}
        aria-busy={startup.isPending || save.isPending || undefined} onCheckedChange={(enabled) => save.mutate(enabled)} />
      {startup.isError || save.isError ? <StateView mode="error" placement="control" title={t("onboarding.startup.unavailable")}
        actions={startup.isError ? <Button variant="ghost" size="sm" onClick={() => void startup.refetch()}>{t("common.tryAgain")}</Button> : undefined} /> : null}
    </PermissionLayout>
  );
}

function PermissionLayout({ icon, title, detail, titleId, children }: {
  icon: IconName; title: string; detail: string; titleId?: string; children: ReactNode;
}) {
  return <div className={styles.row}><span className={styles.icon}><Icon name={icon} size="lg" /></span><div className={styles.copy}><h2 id={titleId}>{title}</h2><p>{detail}</p></div><div className={styles.control}>{children}</div></div>;
}
