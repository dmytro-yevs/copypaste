/**
 * Restarting Shizuku is a normal status, not a failure. Only a refused read
 * gets an alert, and no action is offered for work CopyPaste cannot perform.
 */
import { Icon } from "@/components/ui/icon";

import {
  EmptyState,
  InlineNotice,
  SettingsRow,
  StatusCard,
} from "@/components/shared";
import { Button, Switch } from "@/components/ui";
import { SettingsDisclosure } from "@/features/settings/components/SettingsDisclosure";
import { SettingsGroupSurface } from "@/features/settings/components/SettingsGroupSurface";
import { CaptureLadder } from "@/features/capture/components/CaptureLadder";
import { CapturePhoneOnlyHelp } from "./CapturePhoneOnlyHelp";
import { ToastNotice } from "./ToastNotice";
import {
  type CapturePrimary,
  ladderOf,
  primaryOf,
  capturePresentationOf,
} from "@/features/capture/model";
import {
  useCaptureMutation,
  useCaptureNow,
  useCaptureState,
} from "@/hooks/useCapture";
import { useTranslation } from "@/i18n";
import { longAge } from "@/lib/format";
import {
  type CaptureSnapshot,
  captureArm,
  captureRefresh,
  captureSetEnabled,
} from "@/lib/ipc";
import styles from "./CaptureSetup.module.css";

const PRIMARY_LABEL = {
  arm: "capture.setup.action.arm",
  permission: "capture.setup.action.permission",
  recheck: "capture.setup.action.checkAgain",
} as const satisfies Record<Exclude<CapturePrimary, "none">, string>;

export function CaptureSetupState() {
  const { t } = useTranslation();
  const capture = useCaptureState();

  if (capture.data !== undefined) {
    return <CaptureSetup snapshot={capture.data} />;
  }

  return (
    <div className={styles.emptyState}>
      {capture.isPending ? (
        <EmptyState
          busy
          title={t("capture.loading.title")}
          body={t("capture.loading.body")}
        />
      ) : (
        <EmptyState
          icon="alert"
          title={t("capture.unknown.title")}
          body={t("capture.unknown.body")}
          action={{
            label: t("common.tryAgain"),
            icon: "refresh",
            onClick: () => void capture.refetch(),
          }}
        />
      )}
    </div>
  );
}

export function CaptureSetup({ snapshot }: { snapshot: CaptureSnapshot }) {
  const { t } = useTranslation();
  const managed = snapshot.rung !== "desktop";

  return (
    <div className={styles.content}>
      <h2 className={styles.heading}>{t("capture.title")}</h2>
      <CaptureStateCard snapshot={snapshot} />
      {snapshot.droppedClips > 0 && <Dropped count={snapshot.droppedClips} />}
      {managed && (
        <SettingsGroupSurface>
          {snapshot.shizuku.supported && (
            <EnableRow enabled={snapshot.shizuku.enabled} />
          )}
          <AlwaysOn />
        </SettingsGroupSurface>
      )}
      {managed && snapshot.shizuku.supported && (
        <SettingsDisclosure title={t("capture.help.title")} description={t("capture.help.summary")}>
          <div className={styles.help}>
            <CapturePhoneOnlyHelp snapshot={snapshot} />
            <CaptureLadder rungs={ladderOf(snapshot)} />
          </div>
        </SettingsDisclosure>
      )}
      {managed && snapshot.shizuku.permission && (
        <SettingsDisclosure title={t("capture.options.title")}>
          <ToastNotice suppressed={snapshot.toastSuppressed} />
        </SettingsDisclosure>
      )}
    </div>
  );
}

function CaptureStateCard({ snapshot }: { snapshot: CaptureSnapshot }) {
  const { t } = useTranslation();
  const run = useCaptureMutation();
  const presentation = capturePresentationOf(snapshot.health);
  const primary = primaryOf(snapshot.nextStep);

  const action = primary === "none" ? undefined : (
    <Button
      state={run.isPending ? "loading" : "normal"}
      onClick={() =>
        run.mutate(
          primary === "recheck"
            ? () => captureRefresh()
            : () => captureArm(),
        )
      }
    >
      <Icon name="refresh" size="md"
        aria-hidden="true"
        className={run.isPending ? styles.spinner : undefined}
      />
      {t(PRIMARY_LABEL[primary])}
    </Button>
  );

  return (
    <StatusCard
      status={presentation.tone}
      density="compact"
      title={snapshot.headline}
      detail={snapshot.detail}
      meta={snapshot.lastCaptureAt === null
        ? undefined
        : t("capture.setup.lastSaved", {
            age: longAge(snapshot.lastCaptureAt),
          })}
      action={action}
      role={presentation.role}
      live={presentation.urgency}
      busy={run.isPending}
    />
  );
}

function AlwaysOn() {
  const { t } = useTranslation();
  const now = useCaptureNow();

  return (
    <SettingsRow
      title={t("capture.setup.always.title")}
      description={t("capture.setup.always.body")}
    >
      <Button
        variant="secondary"
        size="sm"
        state={now.isPending ? "loading" : "normal"}
        onClick={() => now.mutate("in_app")}
      >
        <Icon name="copy" size="sm" />
        {t("capture.setup.always.action")}
      </Button>
    </SettingsRow>
  );
}

function EnableRow({ enabled }: { enabled: boolean }) {
  const { t } = useTranslation();
  const run = useCaptureMutation();

  return (
    <SettingsRow
      title={t("capture.setup.enable.title")}
      description={t("capture.setup.enable.body")}
    >
      <Switch
        checked={enabled}
        disabled={run.isPending}
        aria-label={t("capture.setup.enable.title")}
        onCheckedChange={(next) => run.mutate(() => captureSetEnabled(next))}
      />
    </SettingsRow>
  );
}

function Dropped({ count }: { count: number }) {
  const { t } = useTranslation();
  return (
    <InlineNotice role="alert" tone="warning" icon="alert">
      {t("capture.setup.dropped", { count })}
    </InlineNotice>
  );
}
