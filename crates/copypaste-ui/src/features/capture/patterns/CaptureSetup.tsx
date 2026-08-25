/**
 * Restarting Shizuku is a normal status, not a failure. Only a refused read
 * gets an alert, and no action is offered for work CopyPaste cannot perform.
 */
import { Icon } from "@/components/ui/icon";

import { EmptyState, InlineNotice, SettingsRow } from "@/components/shared";
import { Button, Surface, Switch } from "@/components/ui";
import { CaptureLadder } from "@/features/capture/components/CaptureLadder";
import { CapturePhoneOnlyHelp } from "./CapturePhoneOnlyHelp";
import { SourceExclusions } from "./SourceExclusions";
import { ToastNotice } from "./ToastNotice";
import {
  type CapturePrimary,
  ladderOf,
  primaryOf,
  toneOf,
} from "@/features/capture/model";
import {
  useCaptureMutation,
  useCaptureNow,
  useCaptureState,
} from "@/hooks/useCapture";
import {
  useServiceConfig,
  useSetServiceConfig,
} from "@/hooks/useServiceConfig";
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

export function CaptureSetupState({
  mode = "full",
}: {
  mode?: "full" | "supplemental";
} = {}) {
  const { t } = useTranslation();
  const capture = useCaptureState();

  if (capture.data !== undefined) {
    return <CaptureSetup snapshot={capture.data} mode={mode} />;
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

export function CaptureSetup({
  snapshot,
  mode = "full",
}: {
  snapshot: CaptureSnapshot;
  mode?: "full" | "supplemental";
}) {
  const managed = snapshot.rung !== "desktop";
  const supplemental = mode === "supplemental";

  return (
    <div className={styles.content}>
      {!supplemental && <StateCard snapshot={snapshot} />}
      {snapshot.droppedClips > 0 && <Dropped count={snapshot.droppedClips} />}
      {managed && <AlwaysOn />}

      {managed && snapshot.shizuku.supported && (
        <>
          <CapturePhoneOnlyHelp snapshot={snapshot} />
          <CaptureLadder rungs={ladderOf(snapshot)} />
          {!supplemental && (
            <EnableRow enabled={snapshot.health.state !== "disabled"} />
          )}
        </>
      )}

      {managed && !supplemental && <SourceExclusionsPanel />}
      {managed && snapshot.shizuku.permission && (
        <ToastNotice suppressed={snapshot.toastSuppressed} />
      )}
    </div>
  );
}

function SourceExclusionsPanel() {
  const config = useServiceConfig();
  const save = useSetServiceConfig();
  const data = config.data?.config;

  if (!data) return null;
  return (
    <SourceExclusions
      ids={data.excluded_app_bundle_ids}
      disabled={save.isPending}
      collapsible
      onChange={(excluded_app_bundle_ids) =>
        save.mutate({ excluded_app_bundle_ids })
      }
    />
  );
}

function StateCard({ snapshot }: { snapshot: CaptureSnapshot }) {
  const { t } = useTranslation();
  const run = useCaptureMutation();
  const tone = toneOf(snapshot.health);
  const primary = primaryOf(snapshot.nextStep);

  return (
    <Surface asChild elevation="raised" border="subtle" radius="md">
      <section
        role={tone === "fault" ? "alert" : "status"}
        data-tone={tone}
        className={styles.stateCard}
      >
        <div className={styles.stateRow}>
          <span
            aria-hidden="true"
            className={styles.stateDot}
            data-tone={tone}
          />
          <div className={styles.stateCopy}>
            <p className={styles.stateTitle}>{snapshot.headline}</p>
            {snapshot.detail && (
              <p className={styles.stateDetail}>{snapshot.detail}</p>
            )}
            {snapshot.lastCaptureAt !== null && (
              <p className={styles.stateAge}>
                {t("capture.setup.lastSaved", {
                  age: longAge(snapshot.lastCaptureAt),
                })}
              </p>
            )}
          </div>
        </div>

        {primary !== "none" && (
          <div className={styles.actions}>
            <Button
              disabled={run.isPending}
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
              {t(
                run.isPending
                  ? "capture.setup.action.busy"
                  : PRIMARY_LABEL[primary],
              )}
            </Button>
          </div>
        )}
      </section>
    </Surface>
  );
}

function AlwaysOn() {
  const { t } = useTranslation();
  const now = useCaptureNow();

  return (
    <section
      data-settings-search-target={`section:${t("capture.setup.always.title")}`}
      className={styles.alwaysOn}
    >
      <h2 className={styles.alwaysTitle}>{t("capture.setup.always.title")}</h2>
      <p className={styles.alwaysBody}>{t("capture.setup.always.body")}</p>
      <div className={styles.actions}>
        <Button
          variant="secondary"
          disabled={now.isPending}
          onClick={() => now.mutate("in_app")}
        >
          <Icon name="library" size="md" />
          {t("capture.setup.always.action")}
        </Button>
      </div>
    </section>
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
